use std::io::Read;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicBool, Ordering};

use super::*;
use crate::testkit::{Reflection, pool, serve};

#[derive(Default)]
struct Ctx {
    logs: StdMutex<Vec<String>>,
    cancel: AtomicBool,
}

impl TaskContext for Ctx {
    fn log(&self, _: LogLevel, code: &str, params: Value) {
        self.logs.lock().unwrap().push(format!("{code} {params}"));
    }
    fn progress(&self, _: u64, _: u64) {}
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
    fn open_file(&self, _: &str) -> PluginResult<Box<dyn Read + Send>> {
        unreachable!()
    }
    fn file_size(&self, _: &str) -> PluginResult<u64> {
        unreachable!()
    }
}

/// 直接装入示例定义
fn tool() -> (GrpcClient, u64) {
    let tool = GrpcClient::default();
    let token = tool.define(pool(), &Ctx::default()).unwrap().token;
    (tool, token)
}

fn invoke(
    tool: &GrpcClient,
    token: u64,
    addr: std::net::SocketAddr,
    method: &str,
    body: Value,
    extra: Value,
) -> PluginResult<Value> {
    let mut args = json!({
        "token": token,
        "method": format!("/demo.v1.Echo/{method}"),
        "target": { "address": addr.to_string() },
        "json": body.to_string(),
    });
    args.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    tool.run_task("invoke", args, &Ctx::default())
}

/// 轮询到调用结束，返回全部消息与最后一批
fn finish(tool: &GrpcClient, started: &Value) -> (Vec<Value>, Value) {
    let mut messages: Vec<Value> = started["messages"].as_array().unwrap().clone();
    let mut last = started.clone();
    while !last["done"].as_bool().unwrap() {
        last = tool
            .call("poll", json!({ "call": started["call"], "waitMs": 1000 }))
            .unwrap();
        messages.extend(last["messages"].as_array().unwrap().iter().cloned());
    }
    let parsed = messages
        .iter()
        .map(|m| serde_json::from_str(m.as_str().unwrap()).unwrap())
        .collect();
    (parsed, last)
}

#[test]
fn unary_calls_return_messages_metadata_and_status() {
    let addr = serve(Reflection::V1);
    let (tool, token) = tool();
    let ctx = Ctx::default();
    let started = tool
        .run_task(
            "invoke",
            json!({
                "token": token,
                "method": "/demo.v1.Echo/Unary",
                "target": { "address": addr.to_string() },
                "metadata": [{ "key": "X-User", "value": "s3cret-token", "secret": true }],
                "json": r#"{"text": "hi", "id": "9007199254740993"}"#,
            }),
            &ctx,
        )
        .unwrap();
    let (messages, last) = finish(&tool, &started);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["text"], "hi|s3cret-token");
    assert_eq!(messages[0]["id"], "9007199254740993");
    assert_eq!(last["status"]["name"], "OK");
    assert_eq!(last["status"]["local"], false);
    assert!(started["headers"].to_string().contains("x-server"));
    assert_eq!(last["trailers"], json!([["x-trailer", "done"]]));
    // 任务日志只有方法、目标与统计，不含凭据与消息内容
    let logs = ctx.logs.lock().unwrap().join("\n");
    assert!(logs.contains("/demo.v1.Echo/Unary"));
    assert!(!logs.contains("s3cret") && !logs.contains("9007199254740993"));

    // 导出：凭据遮罩，消息与状态完整
    let exported: Value = serde_json::from_str(
        tool.call("export", json!({ "call": started["call"] }))
            .unwrap()
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        exported["metadata"],
        json!([{ "key": "x-user", "value": "••••••" }])
    );
    assert_eq!(exported["messages"][0]["id"], "9007199254740993");
    assert_eq!(exported["status"]["name"], "OK");
    // 凭据只出现在服务器回显的消息里，metadata 部分已遮罩
    assert!(!exported["metadata"].to_string().contains("s3cret"));
}

#[test]
fn business_errors_and_trailers_only_responses() {
    let addr = serve(Reflection::V1);
    let (tool, token) = tool();
    let started = invoke(&tool, token, addr, "Fail", json!({}), json!({})).unwrap();
    let (messages, last) = finish(&tool, &started);
    assert!(messages.is_empty());
    assert_eq!(last["status"]["name"], "INVALID_ARGUMENT");
    assert_eq!(last["status"]["message"], "bad input 你");
    let started = invoke(&tool, token, addr, "Deny", json!({}), json!({})).unwrap();
    assert_eq!(started["done"], true);
    assert_eq!(started["status"]["name"], "PERMISSION_DENIED");
    assert_eq!(started["status"]["message"], "no access");
}

#[test]
fn deadlines_end_calls_with_deadline_exceeded() {
    let addr = serve(Reflection::V1);
    let (tool, token) = tool();
    let started = invoke(
        &tool,
        token,
        addr,
        "Slow",
        json!({}),
        json!({ "deadlineMs": 500 }),
    )
    .unwrap();
    let (_, last) = finish(&tool, &started);
    assert_eq!(last["status"]["name"], "DEADLINE_EXCEEDED");
    assert_eq!(last["status"]["local"], true);
}

#[test]
fn server_streams_arrive_in_batches_and_respect_limits() {
    let addr = serve(Reflection::V1);
    let (tool, token) = tool();
    let started = invoke(
        &tool,
        token,
        addr,
        "Watch",
        json!({ "text": "t", "count": 5, "delayMs": 150 }),
        json!({}),
    )
    .unwrap();
    assert!(
        started["messages"].as_array().unwrap().len() < 5,
        "first batch is incremental"
    );
    let (messages, last) = finish(&tool, &started);
    let indexes: Vec<i64> = messages
        .iter()
        .map(|m| m["index"].as_i64().unwrap_or(0))
        .collect();
    assert_eq!(indexes, [0, 1, 2, 3, 4]);
    assert_eq!(
        (last["count"].as_u64(), last["status"]["name"].as_str()),
        (Some(5), Some("OK"))
    );

    let started = invoke(
        &tool,
        token,
        addr,
        "Watch",
        json!({ "count": 10, "delayMs": 10 }),
        json!({ "maxMessages": 2 }),
    )
    .unwrap();
    let (messages, last) = finish(&tool, &started);
    assert_eq!(messages.len(), 2);
    assert_eq!(last["truncated"], true);
    assert_eq!(last["status"]["name"], "RESOURCE_EXHAUSTED");
}

#[test]
fn calls_can_be_cancelled() {
    let addr = serve(Reflection::V1);
    let (tool, token) = tool();
    let started = invoke(
        &tool,
        token,
        addr,
        "Watch",
        json!({ "count": 100, "delayMs": 100 }),
        json!({}),
    )
    .unwrap();
    let cancelled = tool
        .call("cancel", json!({ "call": started["call"] }))
        .unwrap();
    assert_eq!(cancelled["done"], true);
    assert_eq!(cancelled["status"]["name"], "CANCELLED");
    // 任务被取消时，等待第一批消息的调用以 CANCELLED 结束
    let ctx = Ctx::default();
    ctx.cancel.store(true, Ordering::SeqCst);
    let started = tool
        .run_task(
            "invoke",
            json!({ "token": token, "method": "/demo.v1.Echo/Slow", "target": { "address": addr.to_string() } }),
            &ctx,
        )
        .unwrap();
    assert_eq!(started["status"]["name"], "CANCELLED");
    assert_eq!(
        tool.call("poll", json!({ "call": 999 })).unwrap_err().code,
        "grpc.call_expired"
    );
}

#[test]
fn rejects_unsupported_and_invalid_calls() {
    let addr = serve(Reflection::V1);
    let (tool, token) = tool();
    let err = invoke(&tool, token, addr, "Upload", json!({}), json!({})).unwrap_err();
    assert_eq!(err.code, "grpc.unsupported_streaming");
    let err = invoke(&tool, token, addr, "Unary", json!({ "nope": 1 }), json!({})).unwrap_err();
    assert_eq!(err.code, "grpc.request_invalid");
    let err = invoke(&tool, token + 1, addr, "Unary", json!({}), json!({})).unwrap_err();
    assert_eq!(err.code, "grpc.definition_expired");
    // 连接不到服务
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let dead = closed.local_addr().unwrap();
    drop(closed);
    let err = invoke(&tool, token, dead, "Unary", json!({}), json!({})).unwrap_err();
    assert_eq!(err.code, "net.connect");
}

#[test]
fn reflection_loads_definitions_and_falls_back() {
    for mode in [Reflection::V1, Reflection::V1Alpha] {
        let addr = serve(mode);
        let tool = GrpcClient::default();
        let loaded = tool
            .run_task(
                "reflect",
                json!({ "target": { "address": addr.to_string() } }),
                &Ctx::default(),
            )
            .unwrap();
        let services = loaded["services"].as_array().unwrap();
        assert_eq!(services.len(), 1);
        assert_eq!(services[0]["name"], "demo.v1.Echo");
        // 反射得到的定义同样可以生成草稿并发起调用
        let token = loaded["token"].as_u64().unwrap();
        let draft = tool
            .call(
                "template",
                json!({ "token": token, "method": "/demo.v1.Echo/Unary" }),
            )
            .unwrap();
        assert!(draft["json"].as_str().unwrap().contains("\"id\": \"0\""));
        let started = invoke(
            &tool,
            token,
            addr,
            "Unary",
            json!({ "text": "r" }),
            json!({}),
        )
        .unwrap();
        assert_eq!(finish(&tool, &started).1["status"]["name"], "OK");
    }
    let addr = serve(Reflection::None);
    let err = GrpcClient::default()
        .run_task(
            "reflect",
            json!({ "target": { "address": addr.to_string() } }),
            &Ctx::default(),
        )
        .unwrap_err();
    assert_eq!(err.code, "grpc.reflection_unavailable");
}

#[test]
fn templates_validation_and_sanitized_drafts() {
    let (tool, token) = tool();
    let draft = tool
        .call(
            "template",
            json!({ "token": token, "method": "/demo.v1.Echo/Watch" }),
        )
        .unwrap();
    assert_eq!(draft["input"], "demo.v1.EchoRequest");
    let checked = tool
        .call(
            "validate",
            json!({ "token": token, "method": "/demo.v1.Echo/Unary", "json": r#"{"id": 5}"# }),
        )
        .unwrap();
    assert_eq!(checked["bytes"], 2);
    assert!(checked["json"].as_str().unwrap().contains("\"5\""));
    let saved = tool
        .call(
            "sanitize",
            json!({
                "target": { "address": "x:1" },
                "metadata": [
                    { "key": "authorization", "value": "Bearer t", "secret": true },
                    { "key": "x-env", "value": "dev" }
                ],
                "drafts": { "/a/b": "{}" }
            }),
        )
        .unwrap();
    assert_eq!(saved["metadata"][0]["value"], "");
    assert_eq!(saved["metadata"][1]["value"], "dev");
    assert_eq!(saved["drafts"]["/a/b"], "{}");
    assert_eq!(saved["target"]["address"], "x:1");
}

#[test]
fn manifest_declares_every_function() {
    let tool = GrpcClient::default();
    let functions = &tool.manifest().functions;
    for name in ["load_proto", "load_descriptor", "reflect", "invoke"] {
        assert!(functions[name].task, "{name}");
    }
    for name in [
        "template", "validate", "poll", "cancel", "export", "sanitize",
    ] {
        assert!(!functions[name].task, "{name}");
    }
}
