use super::*;
use serde_json::json;

#[test]
fn send_is_a_task_and_curl_is_a_call() {
    let tool = HttpClient::default();
    assert!(tool.manifest().functions["send"].task);
    let curl = tool
        .call("to_curl", json!({"method": "GET", "url": "https://x.dev"}))
        .unwrap();
    assert_eq!(curl, "curl \\\n  'https://x.dev/' \\\n  -L");
}

#[test]
fn calls_resolve_templates_and_keep_legacy_arguments_compatible() {
    let tool = HttpClient::default();
    let curl = tool
        .call(
            "to_curl",
            json!({
                "method": "GET", "url": "{{base}}/users",
                "params": [{ "key": "token", "value": "{{token}}" }],
                "variables": [
                    { "key": "base", "value": "https://example.com" },
                    { "key": "token", "value": "has spaces & /", "secret": true }
                ]
            }),
        )
        .unwrap();
    assert!(
        curl.as_str()
            .unwrap()
            .contains("https://example.com/users?token=has+spaces+%26+%2F")
    );
    let resolved = tool
        .call(
            "resolve_request",
            json!({
                "request": { "method": "GET", "url": "{{base}}" },
                "variables": [{ "key": "base", "value": "https://example.com" }]
            }),
        )
        .unwrap();
    assert_eq!(resolved["url"], "https://example.com");
    assert_eq!(resolved["timeoutMs"], 30_000);
    assert_eq!(resolved["body"]["kind"], "none");
    let imported = tool
        .call(
            "import_curl",
            json!({ "command": "curl https://example.com" }),
        )
        .unwrap();
    assert_eq!(imported["request"]["url"], "https://example.com");
    assert!(imported["warnings"].is_array());
    assert_ne!(
        tool.call("new_id", json!({})).unwrap(),
        tool.call("new_id", json!({})).unwrap()
    );
}

#[test]
fn workspace_api_rejects_malformed_shapes_and_returns_a_safe_copy() {
    let tool = HttpClient::default();
    let state = json!({
        "version": 1,
        "collections": [], "requests": [],
        "environments": [{
            "id": "env", "name": "Local",
            "variables": [{ "key": "token", "value": "private", "secret": true }]
        }],
        "activeEnvironmentId": "env"
    });
    let saved = tool
        .call("workspace_validate", json!({ "workspace": state.clone() }))
        .unwrap();
    assert_eq!(saved["environments"][0]["variables"][0]["value"], "");
    assert_eq!(state["environments"][0]["variables"][0]["value"], "private");
    for workspace in [
        json!({}),
        json!({ "version": "1", "collections": [], "requests": [], "environments": [] }),
    ] {
        assert_eq!(
            tool.call("workspace_validate", json!({ "workspace": workspace }))
                .unwrap_err()
                .code,
            "plugin.invalid_args"
        );
    }
}

/// /echo 返回收到的 Authorization 头；/same 302 到同源 /echo；/cross 302 到另一个源的 /echo
fn echo_servers() -> (String, String) {
    fn start(redirect: Option<String>) -> String {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let base = format!(
            "http://127.0.0.1:{}",
            server.server_addr().to_ip().unwrap().port()
        );
        let own = base.clone();
        std::thread::spawn(move || {
            for request in server.incoming_requests() {
                let auth = request
                    .headers()
                    .iter()
                    .find(|h| h.field.equiv("Authorization"))
                    .map(|h| h.value.to_string())
                    .unwrap_or_default();
                let path = request
                    .url()
                    .split('?')
                    .next()
                    .unwrap_or_default()
                    .to_owned();
                let location = match path.as_str() {
                    "/same" => Some(format!("{own}/echo")),
                    "/cross" => redirect.clone(),
                    _ => None,
                };
                let response = match location {
                    Some(to) => tiny_http::Response::from_string("")
                        .with_status_code(302)
                        .with_header(tiny_http::Header::from_bytes("Location", to).unwrap()),
                    None => tiny_http::Response::from_string(format!("auth=[{auth}]")),
                };
                let _ = request.respond(response);
            }
        });
        base
    }
    let other = start(None);
    let main = start(Some(format!("{other}/echo")));
    (main, other)
}

struct NoopCtx(std::sync::Mutex<Vec<String>>);

impl TaskContext for NoopCtx {
    fn log(&self, _: tf_plugin_api::LogLevel, code: &str, params: Value) {
        self.0.lock().unwrap().push(format!("{code} {params}"));
    }
    fn progress(&self, _: u64, _: u64) {}
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        false
    }
    fn open_file(&self, _: &str) -> PluginResult<Box<dyn std::io::Read + Send>> {
        unreachable!()
    }
    fn file_size(&self, _: &str) -> PluginResult<u64> {
        unreachable!()
    }
}

#[test]
fn oauth_sessions_inject_bearer_only_for_the_same_origin() {
    let tool = HttpClient::default();
    let (main, _) = echo_servers();
    let ctx = NoopCtx(Default::default());
    let send = |path: &str, extra: Value| {
        let mut args = json!({
            "method": "GET", "url": format!("{main}{path}"),
            "oauth": { "configId": "cfg", "environmentId": "dev" }
        });
        args.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        tool.run_task("send", args, &ctx)
    };
    assert_eq!(
        send("/echo", json!({})).unwrap_err().code,
        "oauth.no_session"
    );

    tool.oauth.insert_for_test("cfg@dev", "token-for-dev", None);
    let reply = send("/echo", json!({})).unwrap();
    assert_eq!(reply["body"], "auth=[Bearer token-for-dev]");
    // 同源重定向保留凭据，跨源重定向不转发
    assert_eq!(
        send("/same", json!({})).unwrap()["body"],
        "auth=[Bearer token-for-dev]"
    );
    assert_eq!(send("/cross", json!({})).unwrap()["body"], "auth=[]");
    // 手写的 Authorization 头与授权冲突
    let err = send(
        "/echo",
        json!({ "headers": [{ "key": "authorization", "value": "Basic x", "enabled": true }] }),
    )
    .unwrap_err();
    assert_eq!(err.code, "oauth.header_conflict");
    // 禁用的头不算冲突
    send(
        "/echo",
        json!({ "headers": [{ "key": "Authorization", "value": "Basic x", "enabled": false }] }),
    )
    .unwrap();
    // 不同环境的会话互相隔离
    let other = tool.run_task(
        "send",
        json!({ "method": "GET", "url": format!("{main}/echo"),
                "oauth": { "configId": "cfg", "environmentId": "prod" } }),
        &ctx,
    );
    assert_eq!(other.unwrap_err().code, "oauth.no_session");
    // 过期的令牌要求先刷新
    tool.oauth.insert_for_test("cfg@dev", "old", Some(1));
    assert_eq!(send("/echo", json!({})).unwrap_err().code, "oauth.expired");
    // 任务日志不含令牌
    let logs = ctx.0.lock().unwrap().join("\n");
    assert!(!logs.contains("token-for-dev"));

    // 会话查询只返回遮罩后的令牌，清除后失效
    tool.oauth
        .insert_for_test("cfg@dev", "token-for-dev-1234", None);
    let views = tool
        .call("oauth_sessions", json!({ "keys": ["cfg@dev", "cfg@prod"] }))
        .unwrap();
    assert_eq!(views.as_array().unwrap().len(), 1);
    assert!(!views.to_string().contains("token-for-dev-1234"));
    let session = json!({ "configId": "cfg", "environmentId": "dev" });
    assert_eq!(
        tool.call("oauth_reveal", session.clone()).unwrap(),
        "token-for-dev-1234"
    );
    assert_eq!(tool.call("oauth_clear", session.clone()).unwrap(), true);
    assert_eq!(
        tool.call("oauth_reveal", session).unwrap_err().code,
        "oauth.no_session"
    );
}

#[test]
fn oauth_functions_are_declared() {
    let tool = HttpClient::default();
    let functions = &tool.manifest().functions;
    for name in [
        "oauth_begin",
        "oauth_cancel",
        "oauth_sessions",
        "oauth_clear",
        "oauth_reveal",
    ] {
        assert!(!functions[name].task, "{name}");
    }
    assert!(functions["oauth_wait"].task && functions["oauth_refresh"].task);
}
