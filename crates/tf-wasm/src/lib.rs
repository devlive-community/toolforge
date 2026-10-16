//! WASM 插件运行时：用 wasmtime 加载符合 `wit/plugin.wit` 的组件，包装为原生 [`ToolPlugin`]，
//! 这样在线安装的插件可以与内置插件一样注册到插件注册表。
//!
//! 隔离：WASI 不开放任何目录、环境变量与网络；读取文件只能通过 `read-file` 宿主函数，
//! 且插件需在 manifest 中声明 `fs:user-selected`。每个插件的内存上限 1 GB，
//! 普通调用 30 秒超时；插件崩溃（trap）后丢弃实例，下次调用重新实例化。

use std::io::{Read, Seek, SeekFrom};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde_json::{Map, Value};
use tf_plugin_api::{
    Detection, LogLevel, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin,
};
use wasmtime::component::{Component, HasSelf, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

wasmtime::component::bindgen!({
    path: "../../wit",
    world: "plugin",
});

use toolforge::plugin::host;

/// epoch 推进间隔
const TICK: Duration = Duration::from_millis(100);
/// 普通调用的超时（按 epoch 计）
const CALL_TICKS: u64 = 300;
/// 任务不设超时（取消由插件轮询 is-cancelled 完成）
const TASK_TICKS: u64 = u64::MAX / 2;
const MEMORY_LIMIT: usize = 1 << 30;
/// 单次 read-file 的上限
const MAX_READ: u32 = 4 * 1024 * 1024;
pub const FS_PERMISSION: &str = "fs:user-selected";

fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.epoch_interruption(true);
        let engine = Engine::new(&config).expect("valid wasmtime config");
        let ticker = engine.clone();
        std::thread::Builder::new()
            .name("wasm-epoch".into())
            .spawn(move || {
                loop {
                    std::thread::sleep(TICK);
                    ticker.increment_epoch();
                }
            })
            .expect("spawn epoch thread");
        engine
    })
}

/// 运行中任务的上下文指针；只在 run_task 持有实例锁期间有效
struct TaskSlot(*const (dyn TaskContext + 'static));

// SAFETY: 指针只在持有实例锁的同一次 run_task 调用中被解引用，调用返回前清空
unsafe impl Send for TaskSlot {}

struct State {
    wasi: WasiCtx,
    table: ResourceTable,
    limits: StoreLimits,
    task: Option<TaskSlot>,
    can_read_files: bool,
}

impl State {
    fn task(&self) -> Option<&dyn TaskContext> {
        // SAFETY: 见 TaskSlot；run_task 返回前一定会把 task 置为 None
        self.task.as_ref().map(|slot| unsafe { &*slot.0 })
    }
}

impl WasiView for State {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

fn failure(code: &str, detail: impl ToString) -> host::Failure {
    host::Failure {
        code: code.to_owned(),
        params: serde_json::json!({ "detail": detail.to_string() }).to_string(),
    }
}

impl host::Host for State {
    fn log(&mut self, level: host::LogLevel, code: String, params: String) {
        if let Some(task) = self.task() {
            let level = match level {
                host::LogLevel::Debug => LogLevel::Debug,
                host::LogLevel::Info => LogLevel::Info,
                host::LogLevel::Warn => LogLevel::Warn,
                host::LogLevel::Error => LogLevel::Error,
            };
            task.log(
                level,
                &code,
                serde_json::from_str(&params).unwrap_or(Value::Null),
            );
        }
    }

    fn progress(&mut self, done: u64, total: u64) {
        if let Some(task) = self.task() {
            task.progress(done, total);
        }
    }

    fn stage(&mut self, code: String) {
        if let Some(task) = self.task() {
            task.stage(&code);
        }
    }

    fn is_cancelled(&mut self) -> bool {
        self.task().is_some_and(|task| task.is_cancelled())
    }

    fn file_size(&mut self, path: String) -> Result<u64, host::Failure> {
        if !self.can_read_files {
            return Err(failure("plugin.permission_denied", FS_PERMISSION));
        }
        let meta = std::fs::metadata(&path).map_err(|e| failure("fs.not_found", e))?;
        if meta.is_dir() {
            return Err(failure("fs.not_a_file", &path));
        }
        Ok(meta.len())
    }

    fn read_file(
        &mut self,
        path: String,
        offset: u64,
        length: u32,
    ) -> Result<Vec<u8>, host::Failure> {
        if !self.can_read_files {
            return Err(failure("plugin.permission_denied", FS_PERMISSION));
        }
        let mut file = std::fs::File::open(&path).map_err(|e| failure("fs.io", e))?;
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| failure("fs.io", e))?;
        let mut buf = Vec::with_capacity(length.min(MAX_READ) as usize);
        file.take(u64::from(length.min(MAX_READ)))
            .read_to_end(&mut buf)
            .map_err(|e| failure("fs.io", e))?;
        Ok(buf)
    }
}

struct Instance {
    store: Store<State>,
    bindings: Plugin,
}

pub struct WasmPlugin {
    manifest: Manifest,
    component: Component,
    linker: Linker<State>,
    instance: Mutex<Option<Instance>>,
    call_ticks: u64,
}

fn load_error(detail: impl ToString) -> PluginError {
    PluginError::new("plugin.load_failed").with("detail", detail.to_string())
}

fn from_failure(f: host::Failure) -> PluginError {
    let mut err = PluginError::new(f.code);
    err.params = serde_json::from_str::<Map<String, Value>>(&f.params).unwrap_or_default();
    err
}

/// trap / 超时等运行时错误
fn trapped(err: wasmtime::Error) -> PluginError {
    if let Some(wasmtime::Trap::Interrupt) = err.downcast_ref::<wasmtime::Trap>() {
        return PluginError::new("plugin.timeout");
    }
    PluginError::new("plugin.crashed").with("detail", format!("{err:#}"))
}

impl WasmPlugin {
    /// 编译组件并检查导出；实例在第一次调用时创建
    pub fn load(manifest: Manifest, wasm: &[u8]) -> PluginResult<Self> {
        let engine = engine();
        let component = Component::new(engine, wasm).map_err(load_error)?;
        let mut linker = Linker::new(engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker).map_err(load_error)?;
        Plugin::add_to_linker::<State, HasSelf<State>>(&mut linker, |state| state)
            .map_err(load_error)?;
        let plugin = Self {
            manifest,
            component,
            linker,
            instance: Mutex::new(None),
            call_ticks: CALL_TICKS,
        };
        // 立即实例化一次，尽早发现缺失的导入或导出
        let instance = plugin.instantiate()?;
        *plugin.instance.lock().unwrap_or_else(|e| e.into_inner()) = Some(instance);
        Ok(plugin)
    }

    /// 修改普通调用的超时（默认 30 秒）
    pub fn with_call_timeout(mut self, timeout: Duration) -> Self {
        self.call_ticks = (timeout.as_millis() / TICK.as_millis()).max(1) as u64;
        self
    }

    fn instantiate(&self) -> PluginResult<Instance> {
        let state = State {
            wasi: WasiCtx::builder().build(),
            table: ResourceTable::new(),
            limits: StoreLimitsBuilder::new().memory_size(MEMORY_LIMIT).build(),
            task: None,
            can_read_files: self.manifest.permissions.iter().any(|p| p == FS_PERMISSION),
        };
        let mut store = Store::new(engine(), state);
        store.limiter(|state| &mut state.limits);
        store.set_epoch_deadline(self.call_ticks);
        store.epoch_deadline_trap();
        let bindings =
            Plugin::instantiate(&mut store, &self.component, &self.linker).map_err(load_error)?;
        Ok(Instance { store, bindings })
    }

    /// 在实例上执行；运行时错误后丢弃实例，下次重新创建
    fn with_instance<T>(
        &self,
        ticks: u64,
        task: Option<&dyn TaskContext>,
        f: impl FnOnce(&mut Instance) -> wasmtime::Result<T>,
    ) -> PluginResult<T> {
        let mut guard = self.instance.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() {
            *guard = Some(self.instantiate()?);
        }
        let instance = guard.as_mut().expect("instance created above");
        instance.store.set_epoch_deadline(ticks);
        // SAFETY: 擦除生命周期后存入 store；本函数返回前（含出错路径）一定清空
        instance.store.data_mut().task = task.map(|ctx| {
            TaskSlot(unsafe {
                std::mem::transmute::<*const dyn TaskContext, *const (dyn TaskContext + 'static)>(
                    ctx,
                )
            })
        });
        let result = f(instance);
        instance.store.data_mut().task = None;
        match result {
            Ok(value) => Ok(value),
            Err(err) => {
                *guard = None;
                Err(trapped(err))
            }
        }
    }

    fn invoke(
        &self,
        function: &str,
        args: Value,
        task: Option<&dyn TaskContext>,
    ) -> PluginResult<Value> {
        let args = args.to_string();
        let ticks = if task.is_some() {
            TASK_TICKS
        } else {
            self.call_ticks
        };
        let output = self.with_instance(ticks, task, |instance| match task {
            Some(_) => instance
                .bindings
                .call_run_task(&mut instance.store, function, &args),
            None => instance
                .bindings
                .call_call(&mut instance.store, function, &args),
        })?;
        let text = output.map_err(from_failure)?;
        serde_json::from_str(&text)
            .map_err(|e| PluginError::new("plugin.invalid_result").with("detail", e.to_string()))
    }
}

impl ToolPlugin for WasmPlugin {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn detect(&self, text: &str) -> Option<Detection> {
        let json = self
            .with_instance(self.call_ticks, None, |instance| {
                instance.bindings.call_detect(&mut instance.store, text)
            })
            .ok()??;
        serde_json::from_str(&json).ok()
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        self.invoke(function, args, None)
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        self.invoke(function, args, Some(ctx))
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
