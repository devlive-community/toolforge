use std::collections::HashMap;
use std::io::Read;
use std::net::{Ipv4Addr, TcpListener};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use super::*;

struct Ctx {
    cancel: AtomicBool,
    logs: Mutex<Vec<String>>,
}

impl Ctx {
    fn new() -> Self {
        Self {
            cancel: AtomicBool::new(false),
            logs: Mutex::default(),
        }
    }
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

/// 本地测试授权服务的令牌端点：记录收到的 code_challenge，按 S256 校验 verifier
struct AuthServer {
    base: String,
    challenge: Arc<Mutex<Option<String>>>,
    fail_refresh: Arc<AtomicBool>,
}

fn form(body: &str) -> HashMap<String, String> {
    url::form_urlencoded::parse(body.as_bytes())
        .into_owned()
        .collect()
}

fn json_response(status: u16, value: Value) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let header = tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap();
    tiny_http::Response::from_string(value.to_string())
        .with_status_code(status)
        .with_header(header)
}

fn auth_server() -> AuthServer {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();
    let challenge: Arc<Mutex<Option<String>>> = Arc::default();
    let fail_refresh = Arc::new(AtomicBool::new(false));
    let (expected, fail) = (challenge.clone(), fail_refresh.clone());
    std::thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let mut body = String::new();
            request.as_reader().read_to_string(&mut body).unwrap();
            let f = form(&body);
            let grant = f.get("grant_type").map(String::as_str);
            let reply = match grant {
                Some("authorization_code") => {
                    let verifier = f.get("code_verifier").cloned().unwrap_or_default();
                    let ok_verifier = expected.lock().unwrap().as_deref()
                        == Some(challenge_of(&verifier).as_str());
                    match f.get("code").map(String::as_str) {
                        _ if !ok_verifier
                            || f.get("client_id").map(String::as_str) != Some("cli") =>
                        {
                            json_response(400, json!({ "error": "invalid_grant" }))
                        }
                        Some("good-code") => json_response(
                            200,
                            json!({ "access_token": "access-0001-secret", "token_type": "bearer",
                                    "expires_in": 3600, "refresh_token": "refresh-1", "scope": "read" }),
                        ),
                        Some("expired-code") => json_response(
                            200,
                            json!({ "access_token": "access-expired-xx", "token_type": "Bearer",
                                    "expires_in": 0 }),
                        ),
                        _ => json_response(400, json!({ "error": "invalid_grant" })),
                    }
                }
                Some("refresh_token") if fail.load(Ordering::SeqCst) => json_response(
                    400,
                    json!({ "error": "invalid_grant", "error_description": "refresh token revoked" }),
                ),
                Some("refresh_token") => match f.get("refresh_token").map(String::as_str) {
                    // 第一次刷新轮换 refresh token，第二次只换 access token
                    Some("refresh-1") => json_response(
                        200,
                        json!({ "access_token": "access-0002-secret", "token_type": "Bearer",
                                "expires_in": 60, "refresh_token": "refresh-2" }),
                    ),
                    Some("refresh-2") => json_response(
                        200,
                        json!({ "access_token": "access-0003-secret", "token_type": "Bearer",
                                "expires_in": 60 }),
                    ),
                    _ => json_response(400, json!({ "error": "invalid_grant" })),
                },
                _ => json_response(400, json!({ "error": "unsupported_grant_type" })),
            };
            let _ = request.respond(reply);
        }
    });
    AuthServer {
        base: format!("http://127.0.0.1:{port}"),
        challenge,
        fail_refresh,
    }
}

fn challenge_of(verifier: &str) -> String {
    challenge(verifier)
}

fn config(server: &AuthServer) -> OAuthConfig {
    OAuthConfig {
        id: "cfg".into(),
        name: "Test".into(),
        authorization_url: format!("{}/authorize?audience=api", server.base),
        token_url: format!("{}/token", server.base),
        client_id: "cli".into(),
        scope: "read".into(),
        redirect_port: None,
        redirect_path: "/callback".into(),
    }
}

fn begin(
    oauth: &OAuth,
    server: &AuthServer,
    env: Option<&str>,
    timeout: Option<u64>,
) -> BeginReply {
    let reply = oauth
        .begin(BeginArgs {
            config: config(server),
            variables: Vec::new(),
            environment_id: env.map(str::to_owned),
            timeout_secs: timeout,
        })
        .unwrap();
    let url = url::Url::parse(&reply.authorize_url).unwrap();
    let q: HashMap<_, _> = url.query_pairs().into_owned().collect();
    *server.challenge.lock().unwrap() = q.get("code_challenge").cloned();
    reply
}

fn param(reply: &BeginReply, name: &str) -> String {
    url::Url::parse(&reply.authorize_url)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.into_owned())
        .unwrap()
}

/// 模拟浏览器访问回调地址
fn callback(reply: &BeginReply, query: &str) -> Result<u16, String> {
    let mut request = HttpRequest::new("GET", &format!("{}?{query}", reply.redirect_uri));
    request.timeout_ms = 3000;
    net::send(&request, None)
        .map(|r| r.status)
        .map_err(|e| e.code)
}

#[test]
fn s256_challenge_matches_an_independent_implementation() {
    // Python: base64.urlsafe_b64encode(hashlib.sha256(v).digest()).rstrip(b"=")
    assert_eq!(
        challenge("dBjftJeZ4CVP-mJ92K9qZTrqqk_rMvz3g9EuT5_MCZQ"),
        "TM6YxXRzDh4QrAgQoXzkaWo6qOREa5GUQzEZ_E5JXI4"
    );
    let a = random_token().unwrap();
    let b = random_token().unwrap();
    assert_ne!(*a, *b);
    // 32 字节随机数 → 43 个字符，满足 verifier 43..=128 的要求
    assert_eq!(a.len(), 43);
}

#[test]
fn completes_the_pkce_flow_and_injects_bearer() {
    let server = auth_server();
    let oauth = OAuth::default();
    let reply = begin(&oauth, &server, Some("dev"), None);
    assert!(reply.redirect_uri.starts_with("http://127.0.0.1:"));
    assert!(reply.redirect_uri.ends_with("/callback"));
    // 保留授权地址原有的查询参数，并带上 PKCE 参数
    assert_eq!(param(&reply, "audience"), "api");
    assert_eq!(param(&reply, "response_type"), "code");
    assert_eq!(param(&reply, "code_challenge_method"), "S256");
    assert_eq!(param(&reply, "redirect_uri"), reply.redirect_uri);
    assert_eq!(param(&reply, "scope"), "read");
    let state = param(&reply, "state");

    assert_eq!(
        callback(&reply, &format!("code=good-code&state={state}")),
        Ok(200)
    );
    let ctx = Ctx::new();
    let view = oauth.wait(reply.flow, &ctx).unwrap();
    assert_eq!(view.key, "cfg@dev");
    assert_eq!(view.scope.as_deref(), Some("read"));
    assert!(view.has_refresh_token && !view.expired);
    assert!(view.expires_at.unwrap() > now_ms());
    assert!(!view.masked.contains("0001"));
    assert_eq!(
        *oauth.bearer("cfg@dev").unwrap(),
        "Bearer access-0001-secret"
    );
    // 日志不含令牌与授权码
    let logs = ctx.logs.lock().unwrap().join("\n");
    assert!(
        !logs.contains("access-0001") && !logs.contains("good-code") && !logs.contains("refresh-1")
    );

    // 回调只接受一次：监听已关闭，重复回调连不上
    assert!(callback(&reply, &format!("code=good-code&state={state}")).is_err());
    // 其他环境看不到这个会话
    assert_eq!(
        oauth.bearer("cfg@prod").unwrap_err().code,
        "oauth.no_session"
    );
    assert_eq!(oauth.reveal("cfg@dev").unwrap(), "access-0001-secret");
    assert!(oauth.clear("cfg@dev"));
    assert_eq!(
        oauth.bearer("cfg@dev").unwrap_err().code,
        "oauth.no_session"
    );
}

#[test]
fn rejects_wrong_state_and_denied_authorizations() {
    let server = auth_server();
    let oauth = OAuth::default();
    let reply = begin(&oauth, &server, None, None);
    // 宿主只负责接收回调，页面总是 200；state 由插件校验
    assert_eq!(callback(&reply, "code=good-code&state=forged"), Ok(200));
    let err = oauth.wait(reply.flow, &Ctx::new()).unwrap_err();
    assert_eq!(err.code, "oauth.state_mismatch");
    assert!(callback(&reply, "code=good-code&state=forged").is_err());

    let reply = begin(&oauth, &server, None, None);
    let state = param(&reply, "state");
    // 路径不对的请求不会结束授权
    let other = BeginReply {
        redirect_uri: reply.redirect_uri.replace("/callback", "/favicon.ico"),
        ..begin_copy(&reply)
    };
    assert_eq!(callback(&other, ""), Ok(404));
    assert_eq!(
        callback(
            &reply,
            &format!("error=access_denied&error_description=User+said+no&state={state}")
        ),
        Ok(200)
    );
    let err = oauth.wait(reply.flow, &Ctx::new()).unwrap_err();
    assert_eq!(err.code, "oauth.denied");
    assert_eq!(err.params["error"], "access_denied");
    assert_eq!(err.params["description"], "User said no");
    assert!(oauth.views(&["cfg@-".into()]).is_empty());
}

fn begin_copy(reply: &BeginReply) -> BeginReply {
    BeginReply {
        flow: reply.flow,
        key: reply.key.clone(),
        authorize_url: reply.authorize_url.clone(),
        redirect_uri: reply.redirect_uri.clone(),
        timeout_secs: reply.timeout_secs,
    }
}

#[test]
fn times_out_and_cancels_releasing_the_listener() {
    let server = auth_server();
    let oauth = OAuth::default();
    let reply = begin(&oauth, &server, None, Some(1));
    assert_eq!(reply.timeout_secs, 1);
    assert_eq!(
        oauth.wait(reply.flow, &Ctx::new()).unwrap_err().code,
        "oauth.timeout"
    );
    assert!(callback(&reply, "code=x&state=y").is_err());

    let reply = begin(&oauth, &server, None, None);
    let ctx = Ctx::new();
    ctx.cancel.store(true, Ordering::SeqCst);
    assert_eq!(
        oauth.wait(reply.flow, &ctx).unwrap_err().code,
        "task.cancelled"
    );
    assert!(callback(&reply, "code=x&state=y").is_err());
    // 已结束的授权不能再等待
    assert_eq!(
        oauth.wait(reply.flow, &Ctx::new()).unwrap_err().code,
        "oauth.flow_expired"
    );

    // 开始新的授权会关闭上一个
    let first = begin(&oauth, &server, None, None);
    let second = begin(&oauth, &server, None, None);
    assert!(callback(&first, "code=x&state=y").is_err());
    assert_eq!(
        oauth.wait(first.flow, &Ctx::new()).unwrap_err().code,
        "oauth.flow_expired"
    );
    oauth.cancel(second.flow);
    assert!(callback(&second, "code=x&state=y").is_err());
}

#[test]
fn refreshes_with_rotation_and_records_failures() {
    let server = auth_server();
    let oauth = OAuth::default();
    let reply = begin(&oauth, &server, None, None);
    let state = param(&reply, "state");
    callback(&reply, &format!("code=good-code&state={state}")).unwrap();
    oauth.wait(reply.flow, &Ctx::new()).unwrap();
    let cfg = config(&server);

    let view = oauth.refresh(&cfg, &[], "cfg@-", &Ctx::new()).unwrap();
    assert!(view.rotated && view.refreshed_at.is_some());
    assert_eq!(*oauth.bearer("cfg@-").unwrap(), "Bearer access-0002-secret");
    // 没有返回新的 refresh token 时沿用轮换后的那个
    let view = oauth.refresh(&cfg, &[], "cfg@-", &Ctx::new()).unwrap();
    assert!(!view.rotated && view.has_refresh_token);
    assert_eq!(view.scope.as_deref(), Some("read"));
    assert_eq!(*oauth.bearer("cfg@-").unwrap(), "Bearer access-0003-secret");

    server.fail_refresh.store(true, Ordering::SeqCst);
    let err = oauth.refresh(&cfg, &[], "cfg@-", &Ctx::new()).unwrap_err();
    assert_eq!(err.code, "oauth.token_error");
    assert_eq!(err.params["description"], "refresh token revoked");
    let view = &oauth.views(&["cfg@-".into()])[0];
    assert_eq!(view.last_error.as_deref(), Some("oauth.token_error"));
    // 失败不清除现有令牌
    assert_eq!(*oauth.bearer("cfg@-").unwrap(), "Bearer access-0003-secret");
    assert_eq!(
        oauth
            .refresh(&cfg, &[], "cfg@other", &Ctx::new())
            .unwrap_err()
            .code,
        "oauth.no_session"
    );
}

#[test]
fn expired_tokens_must_be_refreshed_before_sending() {
    let server = auth_server();
    let oauth = OAuth::default();
    let reply = begin(&oauth, &server, None, None);
    let state = param(&reply, "state");
    callback(&reply, &format!("code=expired-code&state={state}")).unwrap();
    let view = oauth.wait(reply.flow, &Ctx::new()).unwrap();
    assert!(view.expired && !view.has_refresh_token);
    assert_eq!(oauth.bearer("cfg@-").unwrap_err().code, "oauth.expired");
    let err = oauth
        .refresh(&config(&server), &[], "cfg@-", &Ctx::new())
        .unwrap_err();
    assert_eq!(err.code, "oauth.no_refresh_token");
}

#[test]
fn bad_codes_fail_the_exchange_without_leaking_values() {
    let server = auth_server();
    let oauth = OAuth::default();
    let reply = begin(&oauth, &server, None, None);
    let state = param(&reply, "state");
    callback(&reply, &format!("code=wrong-code&state={state}")).unwrap();
    let err = oauth.wait(reply.flow, &Ctx::new()).unwrap_err();
    assert_eq!(err.code, "oauth.token_error");
    assert!(!err.to_string().contains("wrong-code"));
}

#[test]
fn validates_configuration() {
    let server = auth_server();
    let oauth = OAuth::default();
    let mut cfg = config(&server);
    cfg.token_url = "http://example.com/token".into();
    let begin = |cfg: &OAuthConfig| {
        oauth.begin(BeginArgs {
            config: cfg.clone(),
            variables: Vec::new(),
            environment_id: None,
            timeout_secs: None,
        })
    };
    let err = begin(&cfg).unwrap_err();
    assert_eq!(
        (err.code.as_str(), &err.params["field"]),
        ("oauth.insecure_url", &json!("tokenUrl"))
    );
    cfg.token_url = "https://auth.example.com/token".into();
    cfg.client_id = " ".into();
    assert_eq!(begin(&cfg).unwrap_err().code, "oauth.missing_field");
    cfg.client_id = "cli".into();
    cfg.redirect_path = "callback".into();
    assert_eq!(begin(&cfg).unwrap_err().code, "oauth.invalid_path");
    cfg.redirect_path = "/cb".into();
    // 固定端口被占用
    let busy = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    cfg.redirect_port = Some(busy.local_addr().unwrap().port());
    assert_eq!(begin(&cfg).unwrap_err().code, "oauth.port_in_use");
    // 模板变量来自当前环境
    cfg.redirect_port = None;
    cfg.client_id = "{{client}}".into();
    let reply = oauth
        .begin(BeginArgs {
            config: cfg.clone(),
            variables: vec![Variable {
                key: "client".into(),
                value: "from-env".into(),
                enabled: true,
                secret: false,
            }],
            environment_id: Some("e".into()),
            timeout_secs: None,
        })
        .unwrap();
    assert!(reply.authorize_url.contains("client_id=from-env"));
    assert!(reply.redirect_uri.ends_with("/cb"));
    oauth.cancel(reply.flow);
    assert_eq!(mask("short"), "••••••••");
    assert_eq!(mask("abcdefghijklmnop"), "abcd••••••••mnop");
}
