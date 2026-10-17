//! gRPC 调试器（在线安装插件）：从 .proto、FileDescriptorSet 或服务器反射加载服务定义，
//! 按方法生成 ProtoJSON 请求草稿，发起一元与服务端流调用，展示消息、status 与 metadata。
//!
//! 定义解析与浏览完全离线；调用只连接用户指定的目标。请求由宿主以 HTTP/2 流式发送，
//! 调用开始后保存在插件内，界面用短时间的 `poll` 逐批取回消息。
//! metadata 中标记为凭据的值只在当前会话使用：保存的草稿、任务日志与导出都不包含它们。

mod catalog;
mod grpc;
mod message;
mod reflection;
#[cfg(test)]
mod testkit;

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Duration;

use prost_reflect::DescriptorPool;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tf_plugin_api::{
    LogLevel, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, parse_args, to_value,
    unknown_function,
};

use grpc::{Batch, Call, CallOptions, MetadataEntry, Target};

const MANIFEST: &str = include_str!("../../manifest.json");
/// 同时保留的调用数
const MAX_CALLS: usize = 4;
/// 单次 poll 最长等待
const MAX_POLL: Duration = Duration::from_millis(1500);
/// 开始调用后先等一小段，一元调用通常能直接拿到结果
const FIRST_WAIT: Duration = Duration::from_millis(400);
/// 反射使用的截止时间
const REFLECT_DEADLINE_MS: u64 = 15_000;

struct Definition {
    token: u64,
    pool: DescriptorPool,
}

/// 调用及其上下文（用于导出）
struct Session {
    call: Call,
    method: String,
    target: Target,
    metadata: Vec<MetadataEntry>,
    request: String,
}

#[derive(Default)]
struct State {
    definition: Option<Definition>,
    calls: BTreeMap<u64, Session>,
    next: u64,
}

pub struct GrpcClient {
    manifest: Manifest,
    state: Mutex<State>,
}

impl Default for GrpcClient {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
            state: Mutex::default(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProtoArgs {
    path: String,
    #[serde(default)]
    includes: Vec<String>,
}

#[derive(Deserialize)]
struct PathArgs {
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReflectArgs {
    target: Target,
    #[serde(default)]
    metadata: Vec<MetadataEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MethodArgs {
    token: u64,
    method: String,
    #[serde(default)]
    json: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InvokeArgs {
    token: u64,
    method: String,
    target: Target,
    #[serde(default)]
    metadata: Vec<MetadataEntry>,
    #[serde(default)]
    deadline_ms: Option<u64>,
    #[serde(default)]
    json: String,
    #[serde(default)]
    max_messages: Option<usize>,
    #[serde(default)]
    max_bytes: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PollArgs {
    call: u64,
    #[serde(default)]
    wait_ms: Option<u64>,
}

#[derive(Deserialize)]
struct CallArgs {
    call: u64,
}

/// 需要保存的界面草稿：metadata 中凭据的值会被清空，其余字段原样保留
#[derive(Deserialize, Serialize)]
struct Saved {
    #[serde(default)]
    metadata: Vec<MetadataEntry>,
    #[serde(flatten)]
    rest: Map<String, Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Loaded {
    token: u64,
    #[serde(flatten)]
    catalog: catalog::Catalog,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Started {
    call: u64,
    headers: Vec<(String, String)>,
    #[serde(flatten)]
    batch: Batch,
}

/// 导出与界面上看到的 metadata：凭据只显示为遮罩
fn masked(entries: &[MetadataEntry]) -> Vec<Value> {
    entries
        .iter()
        .filter(|e| e.enabled && !e.key.trim().is_empty())
        .map(|e| {
            json!({
                "key": e.key.trim().to_ascii_lowercase(),
                "value": if e.secret { "••••••" } else { e.value.as_str() },
            })
        })
        .collect()
}

impl GrpcClient {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn define(&self, pool: DescriptorPool, ctx: &dyn TaskContext) -> PluginResult<Loaded> {
        let catalog = catalog::catalog(&pool);
        if catalog.services.is_empty() {
            return Err(PluginError::new("grpc.no_services"));
        }
        ctx.log(
            LogLevel::Info,
            "grpc.loaded",
            json!({ "services": catalog.services.len(), "files": catalog.files }),
        );
        let mut state = self.state();
        state.next += 1;
        let token = state.next;
        state.definition = Some(Definition { token, pool });
        Ok(Loaded { token, catalog })
    }

    fn pool(&self, token: u64) -> PluginResult<DescriptorPool> {
        match &self.state().definition {
            Some(d) if d.token == token => Ok(d.pool.clone()),
            _ => Err(PluginError::new("grpc.definition_expired")),
        }
    }

    fn invoke(&self, args: InvokeArgs, ctx: &dyn TaskContext) -> PluginResult<Started> {
        let pool = self.pool(args.token)?;
        let method = catalog::find_method(&pool, &args.method)?;
        if method.is_client_streaming() {
            return Err(
                PluginError::new("grpc.unsupported_streaming").with("method", args.method.as_str())
            );
        }
        let request = message::parse(&method.input(), &args.json)?;
        let path = catalog::method_path(&method);
        let base = args.target.base()?;
        ctx.stage("grpc.calling");
        ctx.log(
            LogLevel::Info,
            "grpc.call",
            json!({ "method": path, "target": base }),
        );
        let opened = grpc::open(
            &CallOptions {
                path: path.clone(),
                target: &args.target,
                metadata: &args.metadata,
                deadline_ms: args.deadline_ms,
                body: message::encode(&request),
            },
            Some(ctx),
        );
        let mut call = match opened {
            Ok((stream, _)) => Call::new(
                0,
                stream,
                method.output(),
                args.max_messages.unwrap_or(grpc::DEFAULT_MAX_MESSAGES),
                args.max_bytes.unwrap_or(grpc::DEFAULT_MAX_BYTES),
            ),
            // 截止时间内连响应头都没拿到：作为 DEADLINE_EXCEEDED 展示，而不是连接错误
            Err(err) if err.code == "grpc.deadline" => {
                return Ok(Started {
                    call: 0,
                    headers: Vec::new(),
                    batch: Batch {
                        messages: Vec::new(),
                        count: 0,
                        bytes: 0,
                        done: true,
                        status: Some(grpc::Status::local(4, "deadline exceeded")),
                        trailers: Vec::new(),
                        elapsed_ms: args.deadline_ms.unwrap_or(grpc::DEFAULT_DEADLINE_MS),
                        truncated: false,
                    },
                });
            }
            Err(err) => return Err(err),
        };
        let batch = call.poll(FIRST_WAIT, Some(ctx));
        if let Some(status) = &batch.status {
            ctx.log(
                LogLevel::Info,
                "grpc.status",
                json!({ "status": status.name, "messages": batch.count, "bytes": batch.bytes }),
            );
        }
        let headers = call.headers.clone();
        let mut state = self.state();
        state.next += 1;
        let id = state.next;
        call.id = id;
        state.calls.insert(
            id,
            Session {
                call,
                method: path,
                target: args.target,
                metadata: args.metadata,
                request: args.json,
            },
        );
        while state.calls.len() > MAX_CALLS {
            let oldest = *state.calls.keys().next().expect("non-empty");
            state.calls.remove(&oldest);
        }
        Ok(Started {
            call: id,
            headers,
            batch,
        })
    }

    fn with_call<T>(&self, id: u64, f: impl FnOnce(&mut Session) -> T) -> PluginResult<T> {
        let mut state = self.state();
        state
            .calls
            .get_mut(&id)
            .map(f)
            .ok_or_else(|| PluginError::new("grpc.call_expired"))
    }

    fn export(&self, id: u64) -> PluginResult<String> {
        self.with_call(id, |s| {
            let call = &s.call;
            let report = json!({
                "method": s.method,
                "target": s.target.base().unwrap_or_default(),
                "metadata": masked(&s.metadata),
                "request": serde_json::from_str::<Value>(&s.request).unwrap_or(Value::String(s.request.clone())),
                "headers": call.headers,
                "messages": call.messages.iter().map(|m| serde_json::from_str::<Value>(m).unwrap_or(Value::Null)).collect::<Vec<_>>(),
                "status": call.status,
                "trailers": call.trailers,
                "count": call.count,
                "bytes": call.bytes,
                "truncated": call.truncated,
            });
            serde_json::to_string_pretty(&report).unwrap_or_default()
        })
    }
}

impl ToolPlugin for GrpcClient {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "template" => {
                let args: MethodArgs = parse_args(args)?;
                let pool = self.pool(args.token)?;
                let method = catalog::find_method(&pool, &args.method)?;
                to_value(json!({
                    "json": message::draft(&method.input()),
                    "input": method.input().full_name(),
                    "output": method.output().full_name(),
                }))
            }
            "validate" => {
                let args: MethodArgs = parse_args(args)?;
                let pool = self.pool(args.token)?;
                let method = catalog::find_method(&pool, &args.method)?;
                let parsed = message::parse(&method.input(), &args.json)?;
                to_value(json!({
                    "bytes": message::encode(&parsed).len(),
                    "json": message::to_json(&parsed)?,
                }))
            }
            "poll" => {
                let args: PollArgs = parse_args(args)?;
                let wait = Duration::from_millis(args.wait_ms.unwrap_or(1000)).min(MAX_POLL);
                to_value(self.with_call(args.call, |s| s.call.poll(wait, None))?)
            }
            "cancel" => {
                let args: CallArgs = parse_args(args)?;
                to_value(self.with_call(args.call, |s| {
                    s.call.cancel();
                    s.call.poll(Duration::ZERO, None)
                })?)
            }
            "export" => {
                let args: CallArgs = parse_args(args)?;
                to_value(self.export(args.call)?)
            }
            "sanitize" => {
                let mut saved: Saved = parse_args(args)?;
                for entry in saved.metadata.iter_mut().filter(|e| e.secret) {
                    entry.value.clear();
                }
                to_value(saved)
            }
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "load_proto" => {
                let args: ProtoArgs = parse_args(args)?;
                ctx.stage("grpc.compiling");
                let pool = catalog::load_proto(&args.path, &args.includes, ctx)?;
                to_value(self.define(pool, ctx)?)
            }
            "load_descriptor" => {
                let args: PathArgs = parse_args(args)?;
                let pool = catalog::load_descriptor(&args.path, ctx)?;
                to_value(self.define(pool, ctx)?)
            }
            "reflect" => {
                let args: ReflectArgs = parse_args(args)?;
                let pool =
                    reflection::reflect(&args.target, &args.metadata, REFLECT_DEADLINE_MS, ctx)?;
                to_value(self.define(pool, ctx)?)
            }
            "invoke" => to_value(self.invoke(parse_args(args)?, ctx)?),
            _ => self.call(function, args),
        }
    }
}

tf_plugin_guest::export_plugin!(GrpcClient);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
