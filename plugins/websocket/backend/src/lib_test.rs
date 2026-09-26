use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value;
use tf_plugin_api::LogLevel;

use super::*;

#[derive(Default)]
struct Ctx {
    cancelled: AtomicBool,
}

impl TaskContext for Ctx {
    fn log(&self, _: LogLevel, _: &str, _: Value) {}
    fn progress(&self, _: u64, _: u64) {}
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
    fn open_file(&self, _: &str) -> PluginResult<Box<dyn Read + Send>> {
        Err(PluginError::new("fs.not_found"))
    }
    fn file_size(&self, _: &str) -> PluginResult<u64> {
        Err(PluginError::new("fs.not_found"))
    }
    fn resource_path(&self, id: &str) -> PluginResult<PathBuf> {
        Err(PluginError::new("resource.missing").with("id", id))
    }
}

/// 回显服务器：原样返回文本与二进制消息，收到 "bye" 时主动关闭
fn echo_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || {
                // 只在客户端请求了子协议时才回应；错误类型由 tungstenite 决定
                #[allow(clippy::result_large_err)]
                let callback =
                    |req: &tungstenite::handshake::server::Request,
                     mut res: tungstenite::handshake::server::Response| {
                        if req.headers().contains_key("sec-websocket-protocol") {
                            res.headers_mut()
                                .insert("sec-websocket-protocol", "chat".parse().unwrap());
                        }
                        Ok(res)
                    };
                let Ok(mut socket) = tungstenite::accept_hdr(stream, callback) else {
                    return;
                };
                while let Ok(message) = socket.read() {
                    match message {
                        tungstenite::Message::Text(text) if text.as_str() == "bye" => {
                            let _ = socket.close(Some(tungstenite::protocol::CloseFrame {
                                code: 4001u16.into(),
                                reason: "done".into(),
                            }));
                        }
                        m @ (tungstenite::Message::Text(_) | tungstenite::Message::Binary(_)) => {
                            let _ = socket.send(m);
                        }
                        _ => {}
                    }
                }
            });
        }
    });
    port
}

/// 等待会话满足条件；超时时先取消连接任务，避免测试线程作用域永远等待
fn wait_until(
    plugin: &WebSocketClient,
    ctx: &Ctx,
    session: &str,
    check: impl Fn(&Value) -> bool,
) -> Value {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        // 连接任务刚启动时会话可能还未创建
        if let Ok(state) = plugin.call("messages", json!({ "session": session }))
            && check(&state)
        {
            return state;
        }
        if Instant::now() > deadline {
            ctx.cancelled.store(true, Ordering::Relaxed);
            let last = plugin.call("messages", json!({ "session": session }));
            panic!("timed out waiting for {session}: {last:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn kinds(state: &Value) -> Vec<String> {
    state["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            format!(
                "{}:{}",
                m["direction"].as_str().unwrap(),
                m["kind"].as_str().unwrap()
            )
        })
        .collect()
}

#[test]
fn exchanges_messages_with_an_echo_server() {
    let port = echo_server();
    let plugin = WebSocketClient::default();
    let ctx = Ctx::default();
    std::thread::scope(|scope| {
        let task = scope.spawn(|| {
            plugin.run_task(
                "connect",
                json!({ "session": "echo", "url": format!("ws://127.0.0.1:{port}/"), "protocols": ["chat"] }),
                &ctx,
            )
        });
        let open = wait_until(&plugin, &ctx, "echo", |s| s["state"] == "open");
        assert_eq!(open["info"]["status"], 101);
        assert_eq!(open["info"]["protocol"], "chat");

        plugin
            .call(
                "send",
                json!({ "session": "echo", "kind": "text", "payload": "héllo" }),
            )
            .unwrap();
        plugin.call("send", json!({ "session": "echo", "kind": "binary", "payload": "de ad be ef", "encoding": "hex" })).unwrap();
        plugin
            .call(
                "send",
                json!({ "session": "echo", "kind": "ping", "payload": "p" }),
            )
            .unwrap();
        let state = wait_until(&plugin, &ctx, "echo", |s| {
            kinds(s).iter().filter(|k| k.starts_with("in:")).count() >= 3
        });
        let messages = state["messages"].as_array().unwrap();
        let echoed = messages
            .iter()
            .find(|m| m["direction"] == "in" && m["kind"] == "text")
            .unwrap();
        assert_eq!(echoed["text"], "héllo");
        let binary = messages
            .iter()
            .find(|m| m["direction"] == "in" && m["kind"] == "binary")
            .unwrap();
        assert_eq!(
            (
                binary["hex"].as_str(),
                binary["base64"].as_str(),
                binary["size"].as_u64()
            ),
            (Some("de ad be ef"), Some("3q2+7w=="), Some(4))
        );
        assert!(kinds(&state).contains(&"in:pong".to_owned()));

        // 只获取新消息
        let last = state["last"].as_u64().unwrap();
        let newer = plugin
            .call("messages", json!({ "session": "echo", "after": last }))
            .unwrap();
        assert!(newer["messages"].as_array().unwrap().is_empty());

        // 服务器主动关闭
        plugin
            .call(
                "send",
                json!({ "session": "echo", "kind": "text", "payload": "bye" }),
            )
            .unwrap();
        let summary = task.join().unwrap().unwrap();
        assert_eq!(summary["closeCode"], 4001);
        assert_eq!(summary["closeReason"], "done");
        let closed = plugin
            .call("messages", json!({ "session": "echo" }))
            .unwrap();
        assert_eq!(closed["state"], "closed");
        assert_eq!(
            plugin
                .call(
                    "send",
                    json!({ "session": "echo", "kind": "text", "payload": "x" })
                )
                .unwrap_err()
                .code,
            "ws.not_open"
        );
    });
}

#[test]
fn closes_from_the_client_and_on_cancel() {
    let port = echo_server();
    let plugin = WebSocketClient::default();
    let ctx = Ctx::default();
    std::thread::scope(|scope| {
        let task = scope.spawn(|| {
            plugin.run_task(
                "connect",
                json!({ "session": "c", "url": format!("ws://localhost:{port}") }),
                &ctx,
            )
        });
        wait_until(&plugin, &ctx, "c", |s| s["state"] == "open");
        plugin
            .call(
                "close",
                json!({ "session": "c", "code": 1000, "reason": "bye" }),
            )
            .unwrap();
        task.join().unwrap().unwrap();
        let state = plugin.call("messages", json!({ "session": "c" })).unwrap();
        assert!(kinds(&state).contains(&"event:close".to_owned()));
    });
    std::thread::scope(|scope| {
        let task = scope.spawn(|| {
            plugin.run_task(
                "connect",
                json!({ "session": "d", "url": format!("ws://127.0.0.1:{port}") }),
                &ctx,
            )
        });
        wait_until(&plugin, &ctx, "d", |s| s["state"] == "open");
        ctx.cancelled.store(true, Ordering::Relaxed);
        task.join().unwrap().unwrap();
        assert_eq!(
            plugin.call("messages", json!({ "session": "d" })).unwrap()["state"],
            "closed"
        );
    });
}

#[test]
fn reports_connection_failures() {
    let plugin = WebSocketClient::default();
    let ctx = Ctx::default();
    // 服务器拒绝升级
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 6\r\n\r\ndenied");
        }
    });
    let err = plugin
        .run_task(
            "connect",
            json!({ "session": "r", "url": format!("ws://127.0.0.1:{port}") }),
            &ctx,
        )
        .unwrap_err();
    assert_eq!(err.code, "ws.rejected");
    assert_eq!(err.params["status"], 403);
    let state = plugin.call("messages", json!({ "session": "r" })).unwrap();
    assert_eq!(state["state"], "closed");
    assert_eq!(state["messages"][0]["code"], "ws.rejected");

    let closed = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let err = plugin
        .run_task(
            "connect",
            json!({ "session": "x", "url": format!("ws://127.0.0.1:{closed}"), "timeoutMs": 1000 }),
            &ctx,
        )
        .unwrap_err();
    assert!(
        matches!(err.code.as_str(), "ws.refused" | "ws.timeout"),
        "{err:?}"
    );
    assert_eq!(
        plugin
            .call("messages", json!({ "session": "missing" }))
            .unwrap_err()
            .code,
        "ws.no_session"
    );
}

#[test]
fn validates_payloads_and_close_codes() {
    assert_eq!(
        decode("DE:AD be ef", Encoding::Hex).unwrap(),
        vec![0xde, 0xad, 0xbe, 0xef]
    );
    assert_eq!(
        decode("3q2+7w==", Encoding::Base64).unwrap(),
        vec![0xde, 0xad, 0xbe, 0xef]
    );
    assert_eq!(
        decode("3q2-7w", Encoding::Base64).unwrap(),
        vec![0xde, 0xad, 0xbe, 0xef]
    );
    assert_eq!(
        decode("zz", Encoding::Hex).unwrap_err().code,
        "ws.invalid_hex"
    );
    assert_eq!(
        decode("!!", Encoding::Base64).unwrap_err().code,
        "ws.invalid_base64"
    );

    let plugin = WebSocketClient::default();
    plugin.sessions.create("v").set_state(State::Open);
    let send = |json: Value| plugin.call("send", json).map(|_| ()).map_err(|e| e.code);
    assert_eq!(
        send(json!({ "session": "v", "kind": "ping", "payload": "x".repeat(126) })),
        Err("ws.ping_too_long".into())
    );
    assert_eq!(
        send(json!({ "session": "v", "kind": "text", "payload": "ff", "encoding": "hex" })),
        Err("ws.not_utf8".into())
    );
    assert_eq!(
        plugin
            .call("close", json!({ "session": "v", "code": 1005 }))
            .unwrap_err()
            .code,
        "ws.invalid_close_code"
    );
    assert_eq!(
        plugin
            .call(
                "close",
                json!({ "session": "v", "reason": "r".repeat(124) })
            )
            .unwrap_err()
            .code,
        "ws.reason_too_long"
    );
}

/// 连接真实的 wss 回显服务：`TOOLFORGE_WS_URL=wss://echo.websocket.org cargo test -p tfp-websocket -- --ignored --nocapture`
#[test]
#[ignore]
fn live_echo() {
    let Some(url) = std::env::var_os("TOOLFORGE_WS_URL") else {
        return;
    };
    let plugin = WebSocketClient::default();
    let ctx = Ctx::default();
    std::thread::scope(|scope| {
        let task = scope.spawn(|| {
            plugin.run_task(
                "connect",
                json!({ "session": "live", "url": url.to_string_lossy() }),
                &ctx,
            )
        });
        let open = wait_until(&plugin, &ctx, "live", |s| s["state"] != "connecting");
        println!("state: {} info: {}", open["state"], open["info"]);
        if open["state"] == "open" {
            plugin
                .call(
                    "send",
                    json!({ "session": "live", "kind": "text", "payload": "toolforge" }),
                )
                .unwrap();
            let state = wait_until(&plugin, &ctx, "live", |s| {
                s["messages"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|m| m["direction"] == "in" && m["text"] == "toolforge")
            });
            println!("{}", state["messages"]);
            plugin.call("close", json!({ "session": "live" })).unwrap();
        }
        println!("{:?}", task.join().unwrap());
    });
}

#[test]
fn formats_json_messages() {
    let plugin = WebSocketClient::default();
    let pretty = plugin
        .call("pretty", json!({ "text": " {\"a\":[1,2]} " }))
        .unwrap();
    assert_eq!(pretty, "{\n  \"a\": [\n    1,\n    2\n  ]\n}");
    assert_eq!(
        plugin.call("pretty", json!({ "text": "hello" })).unwrap(),
        Value::Null
    );
    assert_eq!(
        plugin.call("pretty", json!({ "text": "42" })).unwrap(),
        Value::Null
    );
}
