//! OAuth 2.0 授权码 + PKCE（S256）流程，面向原生公共客户端（RFC 8252 / 7636 / 9700）。
//!
//! - `begin`：生成一次性的 state 与 code_verifier，请宿主在 127.0.0.1 上监听回调，返回授权地址；
//!   由界面通过宿主用系统浏览器打开，登录页面完全由授权服务器提供。
//! - `wait`（任务）：等待回调、校验 state、兑换令牌；拒绝授权、超时、取消都会结束会话并释放监听端口。
//!   宿主的回调监听只接受一次：收到第一个回调后立即关闭，重复回调无法再连上。
//! - `refresh`（任务）：用户主动刷新；服务端轮换 refresh token 时替换旧值。
//!
//! 令牌、授权码与 verifier 只保存在本插件实例的内存中（按「配置 + 环境」隔离），
//! 不进入工作区、任务日志或错误参数；清除授权或退出应用即销毁，重启后需要重新授权。

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use data_encoding::BASE64URL_NOPAD;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext, cancelled};
use tf_plugin_guest::net::{self, Callback, HttpRequest};
use zeroize::Zeroizing;

use crate::workspace::{Variable, resolve_text};

/// 等待回调的默认时长
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_TIMEOUT: Duration = Duration::from_secs(1800);
const POLL: Duration = Duration::from_millis(100);
/// 令牌端点请求的超时（毫秒）
const TOKEN_TIMEOUT_MS: u32 = 30_000;
/// 令牌端点响应的读取上限
const MAX_TOKEN_RESPONSE: u64 = 256 * 1024;
/// 错误描述在界面上显示的最大长度
const MAX_DESCRIPTION: usize = 300;

/// 授权配置：只含非敏感信息，随工作区保存
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OAuthConfig {
    pub id: String,
    pub name: String,
    pub authorization_url: String,
    pub token_url: String,
    pub client_id: String,
    #[serde(default)]
    pub scope: String,
    /// 固定的回调端口；为空时由系统分配
    #[serde(default)]
    pub redirect_port: Option<u16>,
    #[serde(default = "default_path")]
    pub redirect_path: String,
}

fn default_path() -> String {
    "/callback".into()
}

/// 解析模板变量后的配置
struct Resolved {
    authorization_url: url::Url,
    token_url: url::Url,
    client_id: String,
    scope: String,
}

fn is_loopback(url: &url::Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

/// 端点必须是 https；只有本机地址允许 http（便于调试本地授权服务）
fn endpoint(raw: &str, field: &str) -> PluginResult<url::Url> {
    let url = url::Url::parse(raw.trim())
        .map_err(|_| PluginError::new("oauth.invalid_url").with("field", field))?;
    match url.scheme() {
        "https" => Ok(url),
        "http" if is_loopback(&url) => Ok(url),
        _ => Err(PluginError::new("oauth.insecure_url").with("field", field)),
    }
}

pub fn validate_path(path: &str) -> bool {
    path.starts_with('/')
        && path.len() <= 128
        && path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/-._~".contains(&b))
}

impl OAuthConfig {
    fn resolve(&self, variables: &[Variable]) -> PluginResult<Resolved> {
        let text = |value: &str, field: &str| resolve_text(value, variables, field);
        let client_id = text(&self.client_id, "clientId")?;
        if client_id.trim().is_empty() {
            return Err(PluginError::new("oauth.missing_field").with("field", "clientId"));
        }
        if !validate_path(&self.redirect_path) {
            return Err(PluginError::new("oauth.invalid_path"));
        }
        Ok(Resolved {
            authorization_url: endpoint(
                &text(&self.authorization_url, "authorizationUrl")?,
                "authorizationUrl",
            )?,
            token_url: endpoint(&text(&self.token_url, "tokenUrl")?, "tokenUrl")?,
            client_id: client_id.trim().to_owned(),
            scope: text(&self.scope, "scope")?.trim().to_owned(),
        })
    }
}

/// 会话标识：同一配置在不同环境下的令牌互相隔离
pub fn session_key(config_id: &str, environment_id: Option<&str>) -> String {
    format!("{config_id}@{}", environment_id.unwrap_or("-"))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn random_token() -> PluginResult<Zeroizing<String>> {
    let mut bytes = Zeroizing::new([0u8; 32]);
    getrandom::fill(bytes.as_mut())
        .map_err(|e| PluginError::new("oauth.random").with("detail", e.to_string()))?;
    Ok(Zeroizing::new(BASE64URL_NOPAD.encode(bytes.as_ref())))
}

/// RFC 7636 S256：BASE64URL(SHA256(ASCII(code_verifier)))
pub fn challenge(verifier: &str) -> String {
    BASE64URL_NOPAD.encode(&Sha256::digest(verifier.as_bytes()))
}

struct Token {
    access: Zeroizing<String>,
    refresh: Option<Zeroizing<String>>,
    token_type: String,
    scope: Option<String>,
    expires_at: Option<u64>,
    obtained_at: u64,
    refreshed_at: Option<u64>,
    rotated: bool,
    last_error: Option<String>,
}

/// 发给界面的会话状态：令牌只给遮罩后的形式
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub key: String,
    pub token_type: String,
    pub scope: Option<String>,
    pub expires_at: Option<u64>,
    pub obtained_at: u64,
    pub refreshed_at: Option<u64>,
    pub expired: bool,
    pub has_refresh_token: bool,
    /// 最近一次刷新是否轮换了 refresh token
    pub rotated: bool,
    pub masked: String,
    pub last_error: Option<String>,
}

fn mask(token: &str) -> String {
    let chars: Vec<char> = token.chars().collect();
    if chars.len() <= 12 {
        return "•".repeat(8);
    }
    let head: String = chars[..4].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}••••••••{tail}")
}

impl Token {
    fn expired(&self, now: u64) -> bool {
        self.expires_at.is_some_and(|at| at <= now)
    }

    fn view(&self, key: &str) -> SessionView {
        SessionView {
            key: key.to_owned(),
            token_type: self.token_type.clone(),
            scope: self.scope.clone(),
            expires_at: self.expires_at,
            obtained_at: self.obtained_at,
            refreshed_at: self.refreshed_at,
            expired: self.expired(now_ms()),
            has_refresh_token: self.refresh.is_some(),
            rotated: self.rotated,
            masked: mask(&self.access),
            last_error: self.last_error.clone(),
        }
    }
}

/// 进行中的授权：最多一个；drop 时宿主关闭回调监听、释放端口
struct Pending {
    flow: u64,
    key: String,
    state: Zeroizing<String>,
    verifier: Zeroizing<String>,
    redirect_uri: String,
    token_url: url::Url,
    client_id: String,
    callback: Callback,
}

#[derive(Default)]
pub struct OAuth {
    sessions: Mutex<HashMap<String, Token>>,
    pending: Mutex<Option<Pending>>,
    next_flow: AtomicU64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeginArgs {
    pub config: OAuthConfig,
    #[serde(default)]
    pub variables: Vec<Variable>,
    #[serde(default)]
    pub environment_id: Option<String>,
    /// 等待回调的秒数
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BeginReply {
    pub flow: u64,
    pub key: String,
    pub authorize_url: String,
    pub redirect_uri: String,
    pub timeout_secs: u64,
}

#[derive(Deserialize)]
pub struct FlowArgs {
    pub flow: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionArgs {
    pub config_id: String,
    #[serde(default)]
    pub environment_id: Option<String>,
}

impl SessionArgs {
    pub fn key(&self) -> String {
        session_key(&self.config_id, self.environment_id.as_deref())
    }
}

#[derive(Deserialize)]
pub struct SessionsArgs {
    pub keys: Vec<String>,
}

fn truncated(text: &str) -> String {
    text.chars().take(MAX_DESCRIPTION).collect()
}

/// 解析回调：返回授权码，或拒绝 / 不匹配的错误
fn read_callback(query: &str, state: &str) -> PluginResult<Zeroizing<String>> {
    let params: HashMap<String, String> = url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();
    if params.get("state").map(String::as_str) != Some(state) {
        return Err(PluginError::new("oauth.state_mismatch"));
    }
    if let Some(error) = params.get("error") {
        let mut err = PluginError::new("oauth.denied").with("error", truncated(error));
        if let Some(description) = params.get("error_description") {
            err = err.with("description", truncated(description));
        }
        return Err(err);
    }
    match params.get("code") {
        Some(code) if !code.is_empty() => Ok(Zeroizing::new(code.clone())),
        _ => Err(PluginError::new("oauth.no_code")),
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    token_type: Option<String>,
    expires_in: Option<Value>,
    refresh_token: Option<String>,
    scope: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// 向令牌端点提交表单；错误里只保留服务端的错误码与描述，不带请求内容
fn token_request(
    url: &url::Url,
    form: &[(&str, &str)],
    ctx: &dyn TaskContext,
) -> PluginResult<TokenResponse> {
    // 正文含授权码 / refresh token，用 Zeroizing 在发送后清零
    let body = Zeroizing::new(
        url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(form)
            .finish(),
    );
    let mut request = HttpRequest::new("POST", url.as_str());
    request.headers = vec![
        ("Accept".into(), "application/json".into()),
        (
            "Content-Type".into(),
            "application/x-www-form-urlencoded".into(),
        ),
    ];
    request.body = Some(body.as_bytes().to_vec());
    request.timeout_ms = TOKEN_TIMEOUT_MS;
    request.follow_redirects = false;
    request.max_body = MAX_TOKEN_RESPONSE;
    let response = net::send(&request, Some(ctx)).map_err(|e| match e.code.as_str() {
        "net.timeout" => PluginError::new("oauth.token_timeout"),
        "net.too_large" => PluginError::new("oauth.token_response").with("status", 0),
        "task.cancelled" => e,
        _ => {
            let mut mapped = PluginError::new("oauth.network");
            mapped.params = e.params;
            mapped
        }
    })?;
    let status = response.status;
    let bytes = Zeroizing::new(response.body);
    let parsed: TokenResponse = serde_json::from_slice(bytes.as_slice())
        .map_err(|_| PluginError::new("oauth.token_response").with("status", status))?;
    if let Some(error) = &parsed.error {
        let mut err = PluginError::new("oauth.token_error")
            .with("status", status)
            .with("error", truncated(error));
        if let Some(description) = &parsed.error_description {
            err = err.with("description", truncated(description));
        }
        return Err(err);
    }
    if !(200..300).contains(&status) {
        return Err(PluginError::new("oauth.token_response").with("status", status));
    }
    Ok(parsed)
}

fn expires_at(value: Option<&Value>, now: u64) -> Option<u64> {
    let secs = match value? {
        Value::Number(n) => n.as_u64()?,
        Value::String(s) => s.trim().parse().ok()?,
        _ => return None,
    };
    Some(now.saturating_add(secs.saturating_mul(1000)))
}

/// 校验令牌响应：必须有 access_token，类型只支持 Bearer
fn new_token(response: TokenResponse, now: u64) -> PluginResult<Token> {
    let access = response
        .access_token
        .filter(|t| !t.is_empty())
        .ok_or_else(|| PluginError::new("oauth.no_access_token"))?;
    let token_type = response.token_type.unwrap_or_else(|| "Bearer".into());
    if !token_type.eq_ignore_ascii_case("bearer") {
        return Err(PluginError::new("oauth.unsupported_token_type").with("type", token_type));
    }
    Ok(Token {
        access: Zeroizing::new(access),
        refresh: response
            .refresh_token
            .filter(|t| !t.is_empty())
            .map(Zeroizing::new),
        token_type,
        scope: response.scope,
        expires_at: expires_at(response.expires_in.as_ref(), now),
        obtained_at: now,
        refreshed_at: None,
        rotated: false,
        last_error: None,
    })
}

impl OAuth {
    fn sessions(&self) -> std::sync::MutexGuard<'_, HashMap<String, Token>> {
        self.sessions.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn take_pending(&self) -> Option<Pending> {
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }

    /// 开始一次授权：取消之前未完成的授权，请宿主监听回调端口并返回授权地址
    pub fn begin(&self, args: BeginArgs) -> PluginResult<BeginReply> {
        let resolved = args.config.resolve(&args.variables)?;
        drop(self.take_pending());
        let timeout = args
            .timeout_secs
            .map_or(DEFAULT_TIMEOUT, Duration::from_secs)
            .clamp(Duration::from_secs(1), MAX_TIMEOUT);
        let requested = args.config.redirect_port.unwrap_or(0);
        let callback = Callback::listen(requested, &args.config.redirect_path, timeout).map_err(
            |e| match e.code.as_str() {
                "net.port_in_use" => PluginError::new("oauth.port_in_use").with("port", requested),
                "net.invalid_path" => PluginError::new("oauth.invalid_path"),
                _ => {
                    let mut mapped = PluginError::new("oauth.listen_failed");
                    mapped.params = e.params;
                    mapped
                }
            },
        )?;
        let redirect_uri = format!(
            "http://127.0.0.1:{}{}",
            callback.port(),
            args.config.redirect_path
        );

        let state = random_token()?;
        let verifier = random_token()?;
        let mut authorize_url = resolved.authorization_url.clone();
        {
            let mut query = authorize_url.query_pairs_mut();
            query
                .append_pair("response_type", "code")
                .append_pair("client_id", &resolved.client_id)
                .append_pair("redirect_uri", &redirect_uri)
                .append_pair("code_challenge", &challenge(&verifier))
                .append_pair("code_challenge_method", "S256")
                .append_pair("state", &state);
            if !resolved.scope.is_empty() {
                query.append_pair("scope", &resolved.scope);
            }
        }
        let flow = self.next_flow.fetch_add(1, Ordering::Relaxed) + 1;
        let key = session_key(&args.config.id, args.environment_id.as_deref());
        *self.pending.lock().unwrap_or_else(|e| e.into_inner()) = Some(Pending {
            flow,
            key: key.clone(),
            state,
            verifier,
            redirect_uri: redirect_uri.clone(),
            token_url: resolved.token_url,
            client_id: resolved.client_id,
            callback,
        });
        Ok(BeginReply {
            flow,
            key,
            authorize_url: authorize_url.to_string(),
            redirect_uri,
            timeout_secs: timeout.as_secs(),
        })
    }

    /// 取消当前授权（界面关闭对话框等）；drop 时宿主释放回调端口
    pub fn cancel(&self, flow: u64) {
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        if pending.as_ref().is_some_and(|p| p.flow == flow) {
            let taken = pending.take();
            drop(pending);
            drop(taken);
        }
    }

    /// 查看当前授权的回调：收到回调时取出整个授权
    fn poll(&self, flow: u64) -> PluginResult<Option<(Pending, String)>> {
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        let current = match pending.as_ref() {
            Some(p) if p.flow == flow => p,
            _ => return Err(PluginError::new("oauth.flow_expired")),
        };
        let polled = current.callback.poll();
        match polled {
            Ok(None) => Ok(None),
            Ok(Some(query)) => Ok(pending.take().map(|p| (p, query))),
            Err(err) => {
                pending.take();
                Err(if err.code == "net.callback_timeout" {
                    PluginError::new("oauth.timeout")
                } else {
                    err
                })
            }
        }
    }

    /// 等待回调并兑换令牌（任务）
    pub fn wait(&self, flow: u64, ctx: &dyn TaskContext) -> PluginResult<SessionView> {
        ctx.stage("oauth.waiting");
        let (pending, query) = loop {
            if ctx.is_cancelled() {
                self.cancel(flow);
                return Err(cancelled());
            }
            if let Some(done) = self.poll(flow)? {
                break done;
            }
            std::thread::sleep(POLL);
        };
        let Pending {
            key,
            state,
            verifier,
            redirect_uri,
            token_url,
            client_id,
            callback,
            ..
        } = pending;
        // 回调已收到，立即释放端口
        drop(callback);
        let code = read_callback(&query, &state)?;
        ctx.log(LogLevel::Info, "oauth.callback", json!({}));
        ctx.stage("oauth.exchanging");
        let response = token_request(
            &token_url,
            &[
                ("grant_type", "authorization_code"),
                ("code", &code),
                ("redirect_uri", &redirect_uri),
                ("client_id", &client_id),
                ("code_verifier", &verifier),
            ],
            ctx,
        )?;
        let token = new_token(response, now_ms())?;
        ctx.log(
            LogLevel::Info,
            "oauth.obtained",
            json!({ "refresh": token.refresh.is_some() }),
        );
        let view = token.view(&key);
        self.sessions().insert(key, token);
        Ok(view)
    }

    /// 用 refresh token 换新令牌（任务）；失败时在会话上记录错误码
    pub fn refresh(
        &self,
        config: &OAuthConfig,
        variables: &[Variable],
        key: &str,
        ctx: &dyn TaskContext,
    ) -> PluginResult<SessionView> {
        let resolved = config.resolve(variables)?;
        let refresh = {
            let sessions = self.sessions();
            let token = sessions
                .get(key)
                .ok_or_else(|| PluginError::new("oauth.no_session"))?;
            token
                .refresh
                .clone()
                .ok_or_else(|| PluginError::new("oauth.no_refresh_token"))?
        };
        ctx.stage("oauth.refreshing");
        let result = token_request(
            &resolved.token_url,
            &[
                ("grant_type", "refresh_token"),
                ("refresh_token", &refresh),
                ("client_id", &resolved.client_id),
            ],
            ctx,
        )
        .and_then(|response| new_token(response, now_ms()));
        let mut sessions = self.sessions();
        let token = sessions
            .get_mut(key)
            .ok_or_else(|| PluginError::new("oauth.no_session"))?;
        match result {
            Ok(fresh) => {
                let rotated = fresh.refresh.is_some();
                token.access = fresh.access;
                if let Some(next) = fresh.refresh {
                    token.refresh = Some(next);
                }
                token.token_type = fresh.token_type;
                if fresh.scope.is_some() {
                    token.scope = fresh.scope;
                }
                token.expires_at = fresh.expires_at;
                token.refreshed_at = Some(fresh.obtained_at);
                token.rotated = rotated;
                token.last_error = None;
                ctx.log(
                    LogLevel::Info,
                    "oauth.refreshed",
                    json!({ "rotated": rotated }),
                );
                Ok(token.view(key))
            }
            Err(err) => {
                token.last_error = Some(err.code.clone());
                Err(err)
            }
        }
    }

    pub fn views(&self, keys: &[String]) -> Vec<SessionView> {
        let sessions = self.sessions();
        keys.iter()
            .filter_map(|key| sessions.get(key).map(|t| t.view(key)))
            .collect()
    }

    /// 清除授权：令牌在 drop 时清零
    pub fn clear(&self, key: &str) -> bool {
        self.sessions().remove(key).is_some()
    }

    /// 用户明确要求时取出 access token（复制）
    pub fn reveal(&self, key: &str) -> PluginResult<String> {
        self.sessions()
            .get(key)
            .map(|t| t.access.to_string())
            .ok_or_else(|| PluginError::new("oauth.no_session"))
    }

    /// 测试用：直接放入一个会话
    #[cfg(test)]
    pub fn insert_for_test(&self, key: &str, access: &str, expires_at: Option<u64>) {
        let mut token = new_token(
            TokenResponse {
                access_token: Some(access.into()),
                token_type: None,
                expires_in: None,
                refresh_token: None,
                scope: None,
                error: None,
                error_description: None,
            },
            now_ms(),
        )
        .expect("valid token");
        token.expires_at = expires_at;
        self.sessions().insert(key.to_owned(), token);
    }

    /// 发送请求时注入的 Authorization 值；过期时要求用户先刷新
    pub fn bearer(&self, key: &str) -> PluginResult<Zeroizing<String>> {
        let sessions = self.sessions();
        let token = sessions
            .get(key)
            .ok_or_else(|| PluginError::new("oauth.no_session"))?;
        if token.expired(now_ms()) {
            return Err(PluginError::new("oauth.expired"));
        }
        Ok(Zeroizing::new(format!("Bearer {}", token.access.as_str())))
    }
}

#[cfg(test)]
#[path = "oauth_test.rs"]
mod tests;
