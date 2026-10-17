//! tf-wasm 测试夹具：覆盖调用、错误、崩溃、超时、任务日志、读写文件、HTTP 请求与回调监听。

use std::io::Read;

use serde_json::{Value, json};
use tf_plugin_api::{
    Detection, LogLevel, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, cancelled,
};

pub struct Fixture {
    manifest: Manifest,
    calls: std::sync::atomic::AtomicU64,
    callback: std::sync::Mutex<Option<tf_plugin_guest::net::Callback>>,
}

impl Default for Fixture {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(include_str!("../manifest.json")),
            calls: Default::default(),
            callback: Default::default(),
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
            "tz" => Ok(json!(std::env::var("TZ").ok())),
            "fail" => Err(PluginError::new("fixture.failed").with("reason", "asked")),
            "write" => {
                let path = args["path"].as_str().unwrap_or_default();
                let text = args["text"].as_str().unwrap_or_default();
                tf_plugin_guest::net::write_file(path, text.as_bytes())?;
                Ok(Value::Null)
            }
            "listen" => {
                let callback = tf_plugin_guest::net::Callback::listen(
                    0,
                    "/cb",
                    std::time::Duration::from_secs(30),
                )?;
                let port = callback.port();
                *self.callback.lock().unwrap() = Some(callback);
                Ok(json!(port))
            }
            "poll" => match self.callback.lock().unwrap().as_ref() {
                Some(callback) => Ok(json!(callback.poll()?)),
                None => Err(PluginError::new("fixture.no_callback")),
            },
            "close" => {
                self.callback.lock().unwrap().take();
                Ok(Value::Null)
            }
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
            "fetch" => {
                let mut request = tf_plugin_guest::net::HttpRequest::new(
                    "POST",
                    args["url"].as_str().unwrap_or_default(),
                );
                request.headers.push(("X-Fixture".into(), "1".into()));
                request.body = Some(b"ping".to_vec());
                let response = tf_plugin_guest::net::send(&request, Some(ctx))?;
                Ok(json!({
                    "status": response.status,
                    "reason": response.reason,
                    "body": String::from_utf8_lossy(&response.body),
                    "redirects": response.redirects,
                }))
            }
            "stream" => {
                let request = tf_plugin_guest::net::StreamRequest::new(
                    "GET",
                    args["url"].as_str().unwrap_or_default(),
                );
                let mut stream = tf_plugin_guest::net::Stream::open(&request)?;
                let mut text = String::new();
                loop {
                    match stream.read(std::time::Duration::from_secs(5))? {
                        tf_plugin_guest::net::StreamEvent::Data(d) => {
                            text.push_str(&String::from_utf8_lossy(&d));
                        }
                        tf_plugin_guest::net::StreamEvent::Pending => {}
                        tf_plugin_guest::net::StreamEvent::End(_) => break,
                    }
                }
                Ok(json!({ "status": stream.head().status, "text": text }))
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
