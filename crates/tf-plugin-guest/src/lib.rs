//! 插件侧绑定：插件照常实现 [`ToolPlugin`]，再用 [`export_plugin!`] 导出，
//! 即可编译为符合 `wit/plugin.wit` 的 wasm32-wasip2 组件。
//!
//! ```ignore
//! #[derive(Default)]
//! struct MyTool { /* ... */ }
//! impl tf_plugin_api::ToolPlugin for MyTool { /* ... */ }
//! tf_plugin_guest::export_plugin!(MyTool);
//! ```

// 让生成的绑定与 export_plugin! 在本 crate 内外都能用同一路径引用
extern crate self as tf_plugin_guest;

pub use tf_plugin_api;
pub use wit_bindgen;

#[doc(hidden)]
pub mod bindings {
    wit_bindgen::generate!({
        path: "../../wit",
        world: "plugin",
        pub_export_macro: true,
        default_bindings_module: "tf_plugin_guest::bindings",
        runtime_path: "tf_plugin_guest::wit_bindgen::rt",
    });
}

// 以下这些辅助类型/函数只在编译为 wasm32 组件时才会被用到：导入、符号声明、
// Guest 适配器实现全部限定在该 target 下，否则非 wasm32 构建（单元测试、宿主
// 测试同时链接多个插件）会因 unused_imports / dead_code 触发 `-D warnings`，
// 同时 __toolforge_plugin_new 符号在非 wasm32 下本就不存在，声明也必须同样限定，
// 否则链接期报 unresolved external（Windows 的 link.exe 会强制检查，之前在这里栽过）。
#[cfg(target_arch = "wasm32")]
mod wasm_guest {
    use std::io::Read;
    use std::sync::OnceLock;

    use serde_json::{Map, Value};
    use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext, ToolPlugin};

    use super::bindings;
    use bindings::toolforge::plugin::host;

    unsafe extern "Rust" {
        /// 由 [`export_plugin!`] 在插件 crate 中定义
        fn __toolforge_plugin_new() -> Box<dyn ToolPlugin>;
    }

    fn plugin() -> &'static dyn ToolPlugin {
        static PLUGIN: OnceLock<Box<dyn ToolPlugin>> = OnceLock::new();
        // SAFETY: 符号由 export_plugin! 定义，签名与这里声明的一致
        PLUGIN
            .get_or_init(|| unsafe { __toolforge_plugin_new() })
            .as_ref()
    }

    fn failure(err: PluginError) -> host::Failure {
        host::Failure {
            code: err.code,
            params: Value::Object(err.params).to_string(),
        }
    }

    fn from_failure(err: host::Failure) -> PluginError {
        let mut out = PluginError::new(err.code);
        out.params = serde_json::from_str::<Map<String, Value>>(&err.params).unwrap_or_default();
        out
    }

    fn parse(args: &str) -> PluginResult<Value> {
        serde_json::from_str(args)
            .map_err(|e| PluginError::new("plugin.invalid_args").with("detail", e.to_string()))
    }

    fn render(result: PluginResult<Value>) -> Result<String, host::Failure> {
        result.map(|v| v.to_string()).map_err(failure)
    }

    /// 从宿主按块读取文件
    struct HostFile {
        path: String,
        offset: u64,
    }

    impl Read for HostFile {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let want = buf.len().min(1024 * 1024) as u32;
            let chunk = host::read_file(&self.path, self.offset, want)
                .map_err(|e| std::io::Error::other(e.code))?;
            buf[..chunk.len()].copy_from_slice(&chunk);
            self.offset += chunk.len() as u64;
            Ok(chunk.len())
        }
    }

    /// 把宿主导入的函数包装为 TaskContext
    struct HostContext;

    impl TaskContext for HostContext {
        fn log(&self, level: LogLevel, code: &str, params: Value) {
            let level = match level {
                LogLevel::Debug => host::LogLevel::Debug,
                LogLevel::Info => host::LogLevel::Info,
                LogLevel::Warn => host::LogLevel::Warn,
                LogLevel::Error => host::LogLevel::Error,
            };
            host::log(level, code, &params.to_string());
        }

        fn progress(&self, done: u64, total: u64) {
            host::progress(done, total);
        }

        fn stage(&self, code: &str) {
            host::stage(code);
        }

        fn is_cancelled(&self) -> bool {
            host::is_cancelled()
        }

        fn open_file(&self, path: &str) -> PluginResult<Box<dyn Read + Send>> {
            host::file_size(path).map_err(from_failure)?;
            Ok(Box::new(HostFile {
                path: path.to_owned(),
                offset: 0,
            }))
        }

        fn file_size(&self, path: &str) -> PluginResult<u64> {
            host::file_size(path).map_err(from_failure)
        }
    }

    #[doc(hidden)]
    pub struct Adapter;

    impl bindings::Guest for Adapter {
        fn call(function: String, args: String) -> Result<String, host::Failure> {
            render(parse(&args).and_then(|args| plugin().call(&function, args)))
        }

        fn run_task(function: String, args: String) -> Result<String, host::Failure> {
            render(parse(&args).and_then(|args| plugin().run_task(&function, args, &HostContext)))
        }

        fn detect(text: String) -> Option<String> {
            plugin()
                .detect(&text)
                .and_then(|d| serde_json::to_string(&d).ok())
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[doc(hidden)]
pub use wasm_guest::Adapter;

/// 导出插件：类型需实现 [`ToolPlugin`] 与 [`Default`]。
/// 只在编译为 wasm32 时生成导出；本机构建（单元测试、宿主测试同时链接多个插件）不产生同名符号
#[macro_export]
macro_rules! export_plugin {
    ($ty:ty) => {
        #[cfg(target_arch = "wasm32")]
        #[unsafe(no_mangle)]
        pub fn __toolforge_plugin_new() -> Box<dyn $crate::tf_plugin_api::ToolPlugin> {
            Box::new(<$ty as ::core::default::Default>::default())
        }
        // 生成的 export! 只接受标识符，先把适配器引入为本地名称
        #[cfg(target_arch = "wasm32")]
        use $crate::Adapter as __ToolforgeAdapter;
        #[cfg(target_arch = "wasm32")]
        $crate::bindings::export!(__ToolforgeAdapter with_types_in $crate::bindings);
    };
}
