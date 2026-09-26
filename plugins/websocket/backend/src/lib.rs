//! WebSocket 客户端插件后端：连接任务在后台收发消息，前端通过会话 id 轮询消息并提交要发送的内容。

mod connect;
mod session;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, parse_args, to_value,
    unknown_function,
};

use session::{Outbound, Session, State};

const MANIFEST: &str = include_str!("../../manifest.json");
/// 最多保留的会话数
const MAX_SESSIONS: usize = 8;

#[derive(Default)]
struct Registry {
    map: HashMap<String, Arc<Session>>,
    /// 创建顺序，超出上限时移除最早的会话
    order: VecDeque<String>,
}

#[derive(Default)]
struct Sessions {
    items: Mutex<Registry>,
}

impl Sessions {
    fn create(&self, id: &str) -> Arc<Session> {
        let mut guard = self.items.lock().unwrap_or_else(|e| e.into_inner());
        let Registry { map, order } = &mut *guard;
        let session = Arc::new(Session::default());
        map.insert(id.to_owned(), session.clone());
        order.retain(|i| i != id);
        order.push_back(id.to_owned());
        while order.len() > MAX_SESSIONS {
            if let Some(old) = order.pop_front() {
                map.remove(&old);
            }
        }
        session
    }

    fn get(&self, id: &str) -> PluginResult<Arc<Session>> {
        let guard = self.items.lock().unwrap_or_else(|e| e.into_inner());
        guard
            .map
            .get(id)
            .cloned()
            .ok_or_else(|| PluginError::new("ws.no_session"))
    }
}

#[derive(Deserialize)]
struct SessionArgs {
    session: String,
    #[serde(default)]
    after: u64,
}

#[derive(Deserialize, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Encoding {
    #[default]
    Text,
    Hex,
    Base64,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Text,
    Binary,
    Ping,
}

#[derive(Deserialize)]
struct SendArgs {
    session: String,
    kind: Kind,
    #[serde(default)]
    payload: String,
    #[serde(default)]
    encoding: Encoding,
}

#[derive(Deserialize)]
struct CloseArgs {
    session: String,
    #[serde(default = "normal_close")]
    code: u16,
    #[serde(default)]
    reason: String,
}

#[derive(Deserialize)]
struct PrettyArgs {
    text: String,
}

fn normal_close() -> u16 {
    1000
}

/// 按编码把输入转换为字节
fn decode(payload: &str, encoding: Encoding) -> PluginResult<Vec<u8>> {
    match encoding {
        Encoding::Text => Ok(payload.as_bytes().to_vec()),
        Encoding::Hex => {
            let clean: String = payload
                .chars()
                .filter(|c| !c.is_whitespace() && *c != ':')
                .collect();
            data_encoding::HEXLOWER_PERMISSIVE
                .decode(clean.as_bytes())
                .map_err(|_| PluginError::new("ws.invalid_hex"))
        }
        Encoding::Base64 => {
            let clean: String = payload.chars().filter(|c| !c.is_whitespace()).collect();
            data_encoding::BASE64
                .decode(clean.as_bytes())
                .or_else(|_| {
                    data_encoding::BASE64URL_NOPAD.decode(clean.trim_end_matches('=').as_bytes())
                })
                .map_err(|_| PluginError::new("ws.invalid_base64"))
        }
    }
}

pub struct WebSocketClient {
    manifest: Manifest,
    sessions: Sessions,
}

impl Default for WebSocketClient {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
            sessions: Sessions::default(),
        }
    }
}

impl WebSocketClient {
    fn send(&self, args: SendArgs) -> PluginResult<()> {
        let session = self.sessions.get(&args.session)?;
        if session.state() != State::Open {
            return Err(PluginError::new("ws.not_open"));
        }
        let message = match args.kind {
            Kind::Text => match args.encoding {
                Encoding::Text => Outbound::Text(args.payload),
                other => Outbound::Text(
                    String::from_utf8(decode(&args.payload, other)?)
                        .map_err(|_| PluginError::new("ws.not_utf8"))?,
                ),
            },
            Kind::Binary => Outbound::Binary(decode(&args.payload, args.encoding)?),
            Kind::Ping => {
                let bytes = decode(&args.payload, args.encoding)?;
                if bytes.len() > 125 {
                    return Err(PluginError::new("ws.ping_too_long"));
                }
                Outbound::Ping(bytes)
            }
        };
        session.enqueue(message);
        Ok(())
    }
}

impl ToolPlugin for WebSocketClient {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "messages" => {
                let args: SessionArgs = parse_args(args)?;
                let (messages, state, info, last) =
                    self.sessions.get(&args.session)?.since(args.after);
                Ok(json!({ "messages": messages, "state": state, "info": info, "last": last }))
            }
            "send" => {
                self.send(parse_args(args)?)?;
                Ok(Value::Null)
            }
            "close" => {
                let args: CloseArgs = parse_args(args)?;
                if !(args.code == 1000 || (3000..=4999).contains(&args.code)) {
                    return Err(PluginError::new("ws.invalid_close_code"));
                }
                if args.reason.len() > 123 {
                    return Err(PluginError::new("ws.reason_too_long"));
                }
                self.sessions
                    .get(&args.session)?
                    .enqueue(Outbound::Close(args.code, args.reason));
                Ok(Value::Null)
            }
            "pretty" => {
                // 消息详情中格式化 JSON；不是 JSON 时返回 null
                let args: PrettyArgs = parse_args(args)?;
                Ok(serde_json::from_str::<Value>(args.text.trim())
                    .ok()
                    .filter(|v| v.is_object() || v.is_array())
                    .and_then(|v| serde_json::to_string_pretty(&v).ok())
                    .map_or(Value::Null, Value::String))
            }
            "clear" => {
                let args: SessionArgs = parse_args(args)?;
                self.sessions.get(&args.session)?.clear();
                Ok(Value::Null)
            }
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "connect" => {
                let args: connect::Args = parse_args(args)?;
                let session = self.sessions.create(&args.session);
                to_value(connect::run(&args, &session, ctx)?)
            }
            _ => self.call(function, args),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
