use std::net::TcpListener;
use std::time::{Duration, Instant};

use serde_json::json;

use super::*;

#[test]
fn lists_listeners_with_their_processes() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let pid = std::process::id();
    let tool = PortManager::default();
    // 并行测试 fork 的子进程会短暂继承监听器，随后可能在进程查询前退出。
    // 等待同一次快照同时包含当前进程的监听器与进程信息，不能只检查 PID 非空。
    let deadline = Instant::now() + Duration::from_secs(2);
    let out = loop {
        let out = tool.call("list", json!({})).unwrap();
        let own_listener = out["sockets"].as_array().unwrap().iter().any(|socket| {
            socket["port"] == port
                && socket["protocol"] == "tcp"
                && socket["pids"]
                    .as_array()
                    .is_some_and(|pids| pids.iter().any(|owner| *owner == pid))
        });
        let own_process = out["processes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|process| process["pid"] == pid);
        if (own_listener && own_process) || Instant::now() >= deadline {
            break out;
        }
        std::thread::sleep(
            Duration::from_millis(50).min(deadline.saturating_duration_since(Instant::now())),
        );
    };
    let socket = out["sockets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["port"] == port && s["protocol"] == "tcp")
        .expect("own listener");
    assert_eq!(socket["state"], "listen");
    assert!(
        socket["pids"]
            .as_array()
            .unwrap()
            .iter()
            .any(|owner| *owner == pid),
        "listener must belong to this process: {socket}"
    );
    assert!(
        out["processes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["pid"] == pid),
        "listener owner {pid} must have process information: {out}"
    );
    assert_eq!(
        tool.call("nope", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );
}

#[test]
fn filters_by_query_and_protocol() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let tool = PortManager::default();
    let out = tool
        .call(
            "list",
            json!({ "query": port.to_string(), "protocol": "tcp" }),
        )
        .unwrap();
    let sockets = out["sockets"].as_array().unwrap();
    assert!(!sockets.is_empty());
    assert!(sockets.iter().all(|s| s["protocol"] == "tcp" && s["port"].to_string().starts_with(&port.to_string())));
    assert!(out["total"].as_u64().unwrap() >= sockets.len() as u64);
    let none = tool
        .call("list", json!({ "query": "no-process-has-this-name-xyz" }))
        .unwrap();
    assert!(none["sockets"].as_array().unwrap().is_empty());
    assert!(none["processes"].as_array().unwrap().is_empty());
}
