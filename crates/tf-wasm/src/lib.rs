//! WASM 插件运行时：用 wasmtime 加载符合 `wit/plugin.wit` 的组件，包装为原生 [`ToolPlugin`]，
//! 这样在线安装的插件可以与内置插件一样注册到插件注册表。
//!
//! 隔离：WASI 不开放任何目录、环境变量与网络；读写文件只能通过 `read-file` / `write-file` 宿主函数，
//! 且插件需在 manifest 中声明 `fs:user-selected`；HTTP 请求（含流式）与本机回调监听由宿主执行（`tf-net`），
//! 插件需声明 `network`。每个插件的内存上限 1 GB，
//! 普通调用 30 秒超时；插件崩溃（trap）后丢弃实例，下次调用重新实例化。

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde_json::{Map, Value};
use tf_net::{CallbackServer, HttpStream};
use tf_plugin_api::net::{HttpRequest, NETWORK_PERMISSION, StreamEvent, StreamRequest};
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
/// 每个插件实例同时存在的回调监听上限
const MAX_CALLBACKS: usize = 4;
/// 每个插件实例同时打开的流上限
const MAX_STREAMS: usize = 8;
/// 单次读取流的最长等待
const MAX_STREAM_WAIT: u32 = 5_000;

/// 宿主的 IANA 时区名，经 WASI 环境变量 TZ 传给插件（插件内置时区库，读取系统时区会失败）
fn local_time_zone() -> String {
    jiff::tz::TimeZone::system()
        .iana_name()
        .unwrap_or("UTC")
        .to_owned()
}

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

/// 插件文件访问的范围
#[derive(Debug, Clone, Default, PartialEq)]
pub enum FileScope {
    /// 不额外限制：桌面端界面调用，路径来自用户在对话框中的选择
    #[default]
    Any,
    /// 只允许授权目录内（AI Agent 调用）；可写目录同时可读
    Roots {
        read: Vec<PathBuf>,
        write: Vec<PathBuf>,
    },
}

/// 规范化路径：存在时解析符号链接与 `..`；不存在时（将要写入的新文件）规范化其父目录
pub fn normalize_path(path: &Path) -> Option<PathBuf> {
    if let Ok(real) = path.canonicalize() {
        return Some(real);
    }
    let parent = path.parent()?.canonicalize().ok()?;
    Some(parent.join(path.file_name()?))
}

impl FileScope {
    pub fn allows(&self, path: &Path, write: bool) -> bool {
        match self {
            Self::Any => true,
            Self::Roots {
                read,
                write: writable,
            } => {
                let Some(target) = normalize_path(path) else {
                    return false;
                };
                let inside = |roots: &[PathBuf]| roots.iter().any(|r| target.starts_with(r));
                inside(writable) || (!write && inside(read))
            }
        }
    }
}

struct State {
    wasi: WasiCtx,
    table: ResourceTable,
    limits: StoreLimits,
    task: Option<TaskSlot>,
    can_read_files: bool,
    /// 本次调用允许访问的路径范围
    scope: FileScope,
    can_network: bool,
    /// 回调监听；实例丢弃时随之关闭并释放端口
    callbacks: HashMap<u32, CallbackServer>,
    next_callback: u32,
    /// 流式请求；实例丢弃时中止
    streams: HashMap<u32, HttpStream>,
    next_stream: u32,
}

impl State {
    fn within_scope(&self, path: &str, write: bool) -> Result<(), host::Failure> {
        if self.scope.allows(Path::new(path), write) {
            Ok(())
        } else {
            Err(failure("plugin.path_denied", path))
        }
    }

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

fn to_failure(err: PluginError) -> host::Failure {
    host::Failure {
        code: err.code,
        params: Value::Object(err.params).to_string(),
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
        self.within_scope(&path, false)?;
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
        self.within_scope(&path, false)?;
        let mut file = std::fs::File::open(&path).map_err(|e| failure("fs.io", e))?;
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| failure("fs.io", e))?;
        let mut buf = Vec::with_capacity(length.min(MAX_READ) as usize);
        file.take(u64::from(length.min(MAX_READ)))
            .read_to_end(&mut buf)
            .map_err(|e| failure("fs.io", e))?;
        Ok(buf)
    }

    fn write_file(&mut self, path: String, data: Vec<u8>) -> Result<(), host::Failure> {
        if !self.can_read_files {
            return Err(failure("plugin.permission_denied", FS_PERMISSION));
        }
        self.within_scope(&path, true)?;
        tf_net::write_file(&path, &data).map_err(to_failure)
    }

    fn http_send(
        &mut self,
        request: host::HttpRequest,
    ) -> Result<host::HttpResponse, host::Failure> {
        if !self.can_network {
            return Err(failure("plugin.permission_denied", NETWORK_PERMISSION));
        }
        let request = HttpRequest {
            method: request.method,
            url: request.url,
            headers: request.headers,
            body: request.body,
            timeout_ms: request.timeout_ms,
            follow_redirects: request.follow_redirects,
            max_redirects: request.max_redirects,
            use_proxy: request.use_proxy,
            max_body: request.max_body,
        };
        let task = self.task();
        let response = tf_net::send(
            &request,
            &|| task.is_some_and(|t| t.is_cancelled()),
            &|done, total| {
                if let Some(t) = task {
                    t.progress(done, total);
                }
            },
        )
        .map_err(to_failure)?;
        Ok(host::HttpResponse {
            status: response.status,
            reason: response.reason,
            version: response.version,
            url: response.url,
            headers: response.headers,
            body: response.body,
            redirects: response.redirects,
            elapsed_ms: response.elapsed_ms,
        })
    }

    fn callback_listen(
        &mut self,
        port: u16,
        path: String,
        timeout_ms: u32,
    ) -> Result<(u32, u16), host::Failure> {
        if !self.can_network {
            return Err(failure("plugin.permission_denied", NETWORK_PERMISSION));
        }
        if self.callbacks.len() >= MAX_CALLBACKS {
            return Err(failure("net.too_many_listeners", MAX_CALLBACKS));
        }
        let server =
            CallbackServer::listen(port, &path, Duration::from_millis(u64::from(timeout_ms)))
                .map_err(to_failure)?;
        self.next_callback = self.next_callback.wrapping_add(1);
        let id = self.next_callback;
        let port = server.port();
        self.callbacks.insert(id, server);
        Ok((id, port))
    }

    fn callback_poll(&mut self, id: u32) -> Result<Option<String>, host::Failure> {
        let server = self
            .callbacks
            .get(&id)
            .ok_or_else(|| failure("net.callback_closed", id))?;
        server.poll().map_err(to_failure)
    }

    fn callback_close(&mut self, id: u32) {
        self.callbacks.remove(&id);
    }

    fn http_stream_open(
        &mut self,
        request: host::StreamRequest,
    ) -> Result<(u32, host::StreamHead), host::Failure> {
        if !self.can_network {
            return Err(failure("plugin.permission_denied", NETWORK_PERMISSION));
        }
        if self.streams.len() >= MAX_STREAMS {
            return Err(failure("net.too_many_streams", MAX_STREAMS));
        }
        let task = self.task();
        let (stream, head) = HttpStream::open(
            &StreamRequest {
                method: request.method,
                url: request.url,
                headers: request.headers,
                body: request.body,
                timeout_ms: request.timeout_ms,
                http2_prior_knowledge: request.http2_prior_knowledge,
            },
            &|| task.is_some_and(|t| t.is_cancelled()),
        )
        .map_err(to_failure)?;
        self.next_stream = self.next_stream.wrapping_add(1);
        let id = self.next_stream;
        self.streams.insert(id, stream);
        Ok((
            id,
            host::StreamHead {
                status: head.status,
                reason: head.reason,
                version: head.version,
                headers: head.headers,
            },
        ))
    }

    fn http_stream_read(
        &mut self,
        id: u32,
        wait_ms: u32,
    ) -> Result<host::StreamEvent, host::Failure> {
        let stream = self
            .streams
            .get_mut(&id)
            .ok_or_else(|| failure("net.stream_closed", id))?;
        let wait = Duration::from_millis(u64::from(wait_ms.min(MAX_STREAM_WAIT)));
        match stream.read(wait).map_err(to_failure)? {
            StreamEvent::Data(data) => Ok(host::StreamEvent::Data(data)),
            StreamEvent::Pending => Ok(host::StreamEvent::Pending),
            StreamEvent::End(trailers) => Ok(host::StreamEvent::End(trailers)),
        }
    }

    fn http_stream_close(&mut self, id: u32) {
        self.streams.remove(&id);
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
    scope: Mutex<FileScope>,
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
            scope: Mutex::new(FileScope::Any),
        };
        // 立即实例化一次，尽早发现缺失的导入或导出
        let instance = plugin.instantiate()?;
        *plugin.instance.lock().unwrap_or_else(|e| e.into_inner()) = Some(instance);
        Ok(plugin)
    }

    /// 之后的调用只允许访问该范围内的文件（命令行的 MCP 服务按 agent 授权目录设置）
    pub fn set_file_scope(&self, scope: FileScope) {
        *self.scope.lock().unwrap_or_else(|e| e.into_inner()) = scope;
    }

    /// 修改普通调用的超时（默认 30 秒）
    pub fn with_call_timeout(mut self, timeout: Duration) -> Self {
        self.call_ticks = (timeout.as_millis() / TICK.as_millis()).max(1) as u64;
        self
    }

    fn instantiate(&self) -> PluginResult<Instance> {
        let state = State {
            wasi: WasiCtx::builder().env("TZ", local_time_zone()).build(),
            table: ResourceTable::new(),
            limits: StoreLimitsBuilder::new().memory_size(MEMORY_LIMIT).build(),
            task: None,
            can_read_files: self.manifest.permissions.iter().any(|p| p == FS_PERMISSION),
            scope: FileScope::Any,
            can_network: self
                .manifest
                .permissions
                .iter()
                .any(|p| p == NETWORK_PERMISSION),
            callbacks: HashMap::new(),
            next_callback: 0,
            streams: HashMap::new(),
            next_stream: 0,
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
        instance.store.data_mut().scope =
            self.scope.lock().unwrap_or_else(|e| e.into_inner()).clone();
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
