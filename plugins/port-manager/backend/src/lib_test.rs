use std::net::TcpListener;

use serde_json::json;

use super::*;

#[test]
fn lists_listeners_with_their_processes() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let tool = PortManager::default();
    // macOS 上按进程遍历文件描述符来关联端口与进程；并行测试同时开关套接字时
    // 本进程可能被暂时跳过，重试几次直到拿到 PID
    let mut attempt = 0;
    let (out, socket) = loop {
        let out = tool.call("list", json!({})).unwrap();
        let socket = out["sockets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["port"] == port && s["protocol"] == "tcp")
            .expect("own listener")
            .clone();
        attempt += 1;
        if socket["pids"].as_array().is_some_and(|p| !p.is_empty()) || attempt >= 5 {
            break (out, socket);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    assert_eq!(socket["state"], "listen");
    let pid = socket["pids"][0].as_u64().expect("listener has a pid");
    assert!(
        out["processes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["pid"] == pid)
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
