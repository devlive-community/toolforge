//! `tfp mcp`：stdio 上的 MCP 服务（JSON-RPC 2.0，每行一条消息），由 agent 客户端启动。
//! stdout 只用于协议消息，日志写 stderr；不监听任何端口。
//!
//! - tools/list 只列出用户在 ToolForge 设置中授权的插件函数
//! - tools/call 每次都重新读取授权；需要确认时通过客户端的 elicitation 询问用户，
//!   客户端不支持时拒绝，并提示在设置中改为始终允许
//! - 在线插件的文件访问限制在授权目录内；原生内置插件经运行环境读文件时同样检查
//! - 调用记入历史并标明来源 agent，x-sensitive 参数打码

use std::collections::HashMap;
use std::io::{BufRead, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};
use tf_core::{AppError, Resources, Store};
use tf_host::agent::{Decision, Mode, Policy, mask_sensitive};
use tf_plugin_api::interface::{Channel, Interface};
use tf_plugin_api::{LogLevel, Manifest, PluginError, PluginResult, TaskContext};

use crate::args;
use crate::catalog::{Catalog, Loaded, slug};
use crate::context::io_error;
use crate::i18n::Messages;
use crate::mcp_tools;
use crate::run::EXIT_OK;

/// 支持的协议版本，第一个为首选
const PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];
/// 等待用户确认的上限
const CONFIRM_TIMEOUT: Duration = Duration::from_secs(600);
const PROGRESS_INTERVAL: Duration = Duration::from_millis(200);

/// JSON-RPC 错误码
const PARSE_ERROR: i64 = -32700;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// 请求 id 可能是数字或字符串，统一用 JSON 文本作键
fn key(id: &Value) -> String {
    id.to_string()
}

struct Server {
    out: Mutex<Box<dyn Write + Send>>,
    pending: Mutex<HashMap<String, Sender<Value>>>,
    cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
    loaded: Mutex<HashMap<String, Arc<Loaded>>>,
    /// 进行中的 tools/call；输入结束后等它们回复完再退出
    workers: Mutex<Vec<std::thread::JoinHandle<()>>>,
    elicitation: AtomicBool,
    next_id: AtomicU64,
    catalog: Catalog,
    store: Option<Store>,
    lang: &'static str,
}

impl Server {
    fn send(&self, message: &Value) {
        let mut out = lock(&self.out);
        let _ = writeln!(out, "{message}");
        let _ = out.flush();
    }

    fn reply(&self, id: &Value, result: Value) {
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "result": result }));
    }

    fn reply_error(&self, id: &Value, code: i64, message: &str) {
        self.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": code, "message": message }
        }));
    }

    fn notify(&self, method: &str, params: Value) {
        self.send(&json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    /// 向客户端发请求并等待回应；超时或出错时返回 None
    fn request(&self, method: &str, params: Value) -> Option<Value> {
        let id = json!(format!(
            "tfp-{}",
            self.next_id.fetch_add(1, Ordering::Relaxed)
        ));
        let (tx, rx) = mpsc::channel();
        lock(&self.pending).insert(key(&id), tx);
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let answer = rx.recv_timeout(CONFIRM_TIMEOUT).ok();
        lock(&self.pending).remove(&key(&id));
        answer
    }

    /// 每次调用都重新读取授权，设置页的修改立即生效；读不到数据库时一律拒绝
    fn policy(&self) -> Policy {
        self.store
            .as_ref()
            .and_then(|store| Policy::load(store).ok())
            .unwrap_or_else(|| Policy::new(false, Vec::new(), &[]))
    }

    fn load(&self, plugin: &str) -> Result<Arc<Loaded>, AppError> {
        if let Some(loaded) = lock(&self.loaded).get(plugin) {
            return Ok(loaded.clone());
        }
        let loaded = Arc::new(self.catalog.load(plugin, self.lang)?);
        lock(&self.loaded).insert(plugin.to_owned(), loaded.clone());
        Ok(loaded)
    }

    fn initialize(&self, params: &Value) -> Value {
        let requested = params["protocolVersion"].as_str().unwrap_or_default();
        let version = PROTOCOL_VERSIONS
            .iter()
            .find(|v| **v == requested)
            .unwrap_or(&PROTOCOL_VERSIONS[0]);
        self.elicitation.store(
            params.pointer("/capabilities/elicitation").is_some(),
            Ordering::Relaxed,
        );
        json!({
            "protocolVersion": version,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": {
                "name": "toolforge",
                "title": "ToolForge",
                "version": env!("CARGO_PKG_VERSION")
            },
            "instructions": Messages::new(self.lang).text("mcp.instructions")
        })
    }

    fn tools(&self) -> Value {
        let policy = self.policy();
        let mut tools = Vec::new();
        if policy.enabled() {
            for (manifest, locale) in self.catalog.manifests(self.lang, Channel::Agent) {
                if !policy.plugin_enabled(&manifest.id) {
                    continue;
                }
                let name = slug(&manifest.id);
                let native = self.catalog.is_builtin(name);
                let messages = Messages::new(self.lang).with_plugin(locale);
                let plugin_name = messages.manifest_text(&manifest.name);
                for (function, spec) in &manifest.functions {
                    let declaration = spec.declaration();
                    let Some(interface) = declaration.exposed(Channel::Agent) else {
                        continue;
                    };
                    if policy.function_mode(&manifest.id, function, interface, native) == Mode::Off
                    {
                        continue;
                    }
                    tools.push(mcp_tools::tool(
                        name,
                        &plugin_name,
                        function,
                        interface,
                        &messages,
                    ));
                }
            }
        }
        json!({ "tools": tools })
    }

    /// 通过客户端的 elicitation 请用户确认；客户端不支持或用户没有接受时返回 false
    fn confirm(
        &self,
        messages: &Messages,
        manifest: &Manifest,
        interface: &Interface,
        args: &Value,
    ) -> bool {
        if !self.elicitation.load(Ordering::Relaxed) {
            return false;
        }
        let message = messages.text_with(
            "mcp.confirm",
            &[
                ("plugin", messages.manifest_text(&manifest.name).into()),
                (
                    "function",
                    messages.manifest_text(&interface.summary).into(),
                ),
                (
                    "args",
                    mask_sensitive(&interface.params, args).to_string().into(),
                ),
            ],
        );
        let params = json!({
            "message": message,
            "requestedSchema": { "type": "object", "properties": {} }
        });
        self.request("elicitation/create", params)
            .is_some_and(|answer| answer["action"] == "accept")
    }

    /// 执行一次工具调用；Err 为协议层错误（未知工具等），执行失败作为工具结果返回
    fn call(&self, params: &Value, cancel: &AtomicBool) -> Result<Value, (i64, String)> {
        let name = params["name"].as_str().unwrap_or_default();
        let unknown = || (INVALID_PARAMS, format!("Unknown tool: {name}"));
        let (plugin, function) = mcp_tools::parse_tool_name(name).ok_or_else(unknown)?;
        let loaded = self.load(plugin).map_err(|_| unknown())?;
        let manifest = loaded.plugin.manifest().clone();
        let spec = manifest.functions.get(function).ok_or_else(unknown)?;
        let declaration = spec.declaration();
        let interface = declaration.exposed(Channel::Agent).ok_or_else(unknown)?;
        let messages = Messages::new(self.lang).with_plugin(loaded.locale.clone());
        let fail = |err: AppError| {
            Ok(mcp_tools::failure(
                &messages.error(&err.code, &err.params),
                &err.code,
            ))
        };

        let arguments = match params.get("arguments") {
            None | Some(Value::Null) => Map::new(),
            Some(Value::Object(map)) => map.clone(),
            Some(_) => return fail(AppError::new("cli.args_json")),
        };
        let parsed = match args::complete(&interface.params, arguments, None) {
            Ok(parsed) => parsed,
            Err(err) => return fail(err),
        };

        let policy = self.policy();
        let native = loaded.wasm.is_none();
        let paths: Vec<(PathBuf, bool)> = parsed
            .paths
            .iter()
            .map(|p| (p.path.clone(), p.write))
            .collect();
        match policy.decide(&manifest.id, function, interface, native, &paths) {
            Decision::Allow => {}
            Decision::Deny(err) => return fail(err),
            Decision::Ask => {
                if !self.confirm(&messages, &manifest, interface, &parsed.args) {
                    let code = if self.elicitation.load(Ordering::Relaxed) {
                        "agent.declined"
                    } else {
                        "agent.needs_confirmation"
                    };
                    return fail(AppError::new(code));
                }
            }
        }

        if let Some(wasm) = &loaded.wasm {
            wasm.set_file_scope(policy.file_scope());
        }
        let result = if spec.task {
            let context = McpContext {
                server: self,
                messages: &messages,
                policy: &policy,
                cancel,
                token: params.pointer("/_meta/progressToken").cloned(),
                plugin_id: manifest.id.clone(),
                resources: tf_host::app_data_dir().map(|d| Resources::new(d.join("resources"))),
                progress: Mutex::new((String::new(), None)),
            };
            loaded
                .plugin
                .run_task(function, parsed.args.clone(), &context)
        } else {
            loaded.plugin.call(function, parsed.args.clone())
        };
        match result {
            Ok(value) => {
                if let Some(store) = &self.store {
                    let masked = mask_sensitive(&interface.params, &parsed.args);
                    tf_host::history::record(store, "agent", &manifest, function, &masked, &value);
                }
                Ok(mcp_tools::success(&value))
            }
            Err(err) => fail(AppError::from(err)),
        }
    }

    fn handle(self: &Arc<Self>, message: Value) {
        let id = message.get("id").cloned();
        match (message["method"].as_str(), id) {
            // 客户端对我们请求（elicitation）的回应
            (None, Some(id)) => {
                if let Some(tx) = lock(&self.pending).remove(&key(&id)) {
                    let _ = tx.send(message.get("result").cloned().unwrap_or(Value::Null));
                }
            }
            (Some("initialize"), Some(id)) => self.reply(&id, self.initialize(&message["params"])),
            (Some("ping"), Some(id)) => self.reply(&id, json!({})),
            (Some("tools/list"), Some(id)) => self.reply(&id, self.tools()),
            (Some("tools/call"), Some(id)) => {
                let cancel = Arc::new(AtomicBool::new(false));
                lock(&self.cancels).insert(key(&id), cancel.clone());
                let server = self.clone();
                let params = message["params"].clone();
                let worker = std::thread::spawn(move || {
                    let outcome = server.call(&params, &cancel);
                    lock(&server.cancels).remove(&key(&id));
                    match outcome {
                        Ok(result) => server.reply(&id, result),
                        Err((code, text)) => server.reply_error(&id, code, &text),
                    }
                });
                let mut workers = lock(&self.workers);
                workers.retain(|w| !w.is_finished());
                workers.push(worker);
            }
            (Some("notifications/cancelled"), None) => {
                if let Some(cancel) = lock(&self.cancels).get(&key(&message["params"]["requestId"]))
                {
                    cancel.store(true, Ordering::Relaxed);
                }
            }
            (Some(method), Some(id)) => {
                self.reply_error(
                    &id,
                    METHOD_NOT_FOUND,
                    &format!("Method not found: {method}"),
                );
            }
            _ => {}
        }
    }
}

/// 任务函数的运行环境：进度经 notifications/progress 上报，取消来自 notifications/cancelled；
/// 原生内置插件经这里读文件时也只能读授权目录
struct McpContext<'a> {
    server: &'a Server,
    messages: &'a Messages,
    policy: &'a Policy,
    cancel: &'a AtomicBool,
    token: Option<Value>,
    plugin_id: String,
    resources: Option<Resources>,
    /// 当前阶段文案与上次上报的时间
    progress: Mutex<(String, Option<Instant>)>,
}

impl McpContext<'_> {
    fn check(&self, path: &str) -> PluginResult<()> {
        if self.policy.path_allowed(std::path::Path::new(path), false) {
            Ok(())
        } else {
            Err(PluginError::new("plugin.path_denied").with("detail", path))
        }
    }
}

impl TaskContext for McpContext<'_> {
    fn log(&self, level: LogLevel, code: &str, params: Value) {
        if matches!(level, LogLevel::Warn | LogLevel::Error) {
            let params = params.as_object().cloned().unwrap_or_default();
            let text = self
                .messages
                .get(&format!("logs.{code}"), &params)
                .unwrap_or_else(|| code.to_owned());
            eprintln!("{text}");
        }
    }

    fn progress(&self, done: u64, total: u64) {
        let Some(token) = &self.token else {
            return;
        };
        let mut progress = lock(&self.progress);
        if progress.1.is_some_and(|t| t.elapsed() < PROGRESS_INTERVAL) && done < total {
            return;
        }
        progress.1 = Some(Instant::now());
        let mut params = json!({ "progressToken": token, "progress": done, "total": total });
        if !progress.0.is_empty() {
            params["message"] = Value::String(progress.0.clone());
        }
        self.server.notify("notifications/progress", params);
    }

    fn stage(&self, code: &str) {
        lock(&self.progress).0 = self
            .messages
            .get(&format!("stages.{code}"), &Map::new())
            .unwrap_or_else(|| code.to_owned());
    }

    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    fn open_file(&self, path: &str) -> PluginResult<Box<dyn Read + Send>> {
        self.check(path)?;
        std::fs::File::open(path)
            .map(|f| Box::new(f) as Box<dyn Read + Send>)
            .map_err(|e| io_error(&e, path))
    }

    fn file_size(&self, path: &str) -> PluginResult<u64> {
        self.check(path)?;
        std::fs::metadata(path)
            .map(|m| m.len())
            .map_err(|e| io_error(&e, path))
    }

    fn resource_path(&self, id: &str) -> PluginResult<PathBuf> {
        self.resources
            .as_ref()
            .and_then(|resources| resources.path(&self.plugin_id, id))
            .ok_or_else(|| PluginError::new("resource.missing").with("id", id))
    }
}

fn server(
    catalog: Catalog,
    store: Option<Store>,
    lang: &'static str,
    out: Box<dyn Write + Send>,
) -> Arc<Server> {
    Arc::new(Server {
        out: Mutex::new(out),
        pending: Mutex::new(HashMap::new()),
        cancels: Mutex::new(HashMap::new()),
        loaded: Mutex::new(HashMap::new()),
        workers: Mutex::new(Vec::new()),
        elicitation: AtomicBool::new(false),
        next_id: AtomicU64::new(1),
        catalog,
        store,
        lang,
    })
}

/// 逐行读取并处理消息，直到输入结束
fn run(server: &Arc<Server>, input: impl BufRead) {
    for line in input.lines() {
        let Ok(line) = line else {
            break;
        };
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(&line) {
            Ok(message) => server.handle(message),
            Err(_) => server.reply_error(&Value::Null, PARSE_ERROR, "Parse error"),
        }
    }
}

pub fn serve(lang: &'static str) -> i32 {
    let store = tf_host::app_data_dir().and_then(|dir| Store::open(&dir.join("toolforge.db")).ok());
    if store.is_none() {
        eprintln!("{}", Messages::new(lang).error("cli.data_dir", &Map::new()));
    }
    let server = server(Catalog::open(), store, lang, Box::new(std::io::stdout()));
    run(&server, std::io::stdin().lock());
    let workers: Vec<_> = lock(&server.workers).drain(..).collect();
    for worker in workers {
        let _ = worker.join();
    }
    EXIT_OK
}

#[cfg(test)]
#[path = "mcp_test.rs"]
mod tests;
