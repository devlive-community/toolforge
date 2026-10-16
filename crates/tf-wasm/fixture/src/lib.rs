//! tf-wasm 测试夹具：覆盖调用、错误、崩溃、超时、任务日志与读取文件。

use std::io::Read;

use serde_json::{Value, json};
use tf_plugin_api::{
    Detection, LogLevel, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, cancelled,
};

pub struct Fixture {
    manifest: Manifest,
    calls: std::sync::atomic::AtomicU64,
}

impl Default for Fixture {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(include_str!("../manifest.json")),
            calls: Default::default(),
        }
    }
}

impl ToolPlugin for Fixture {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn detect(&self, text: &str) -> Option<Detection> {
        (text == "hello").then(|| Detection::new(77, "greeting"))
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        let calls = self
            .calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        match function {
            "echo" => Ok(json!({ "args": args, "calls": calls })),
            "sum" => {
                Ok(json!(args["values"].as_array().map_or(0, |v| {
                    v.iter().filter_map(Value::as_i64).sum::<i64>()
                })))
            }
            "fail" => Err(PluginError::new("fixture.failed").with("reason", "asked")),
            "panic" => panic!("boom"),
            "spin" => loop {
                std::hint::black_box(calls);
            },
            other => Err(PluginError::new("plugin.function_not_found").with("function", other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "count" => {
                let total = args["total"].as_u64().unwrap_or(3);
                ctx.stage("fixture.counting");
                for i in 1..=total {
                    if ctx.is_cancelled() {
                        return Err(cancelled());
                    }
                    ctx.log(LogLevel::Info, "fixture.tick", json!({ "i": i }));
                    ctx.progress(i, total);
                }
                Ok(json!({ "counted": total }))
            }
            "read" => {
                let path = args["path"].as_str().unwrap_or_default();
                let size = ctx.file_size(path)?;
                let mut text = String::new();
                ctx.open_file(path)?
                    .read_to_string(&mut text)
                    .map_err(|e| PluginError::new("fs.io").with("detail", e.to_string()))?;
                Ok(json!({ "size": size, "text": text }))
            }
            other => self.call(other, args),
        }
    }
}

tf_plugin_guest::export_plugin!(Fixture);
