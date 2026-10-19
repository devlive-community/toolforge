//! 逐阶段判定：请求分析（同源 / safelist / forbidden）→ OPTIONS 预检草稿 →
//! 预检响应 CORS check → 实际响应 CORS check 与可读头清单。
//! 预检失败时实际请求不会发出；实际响应缺失时给出「尚无法判断」，不伪造结论。

use serde::Serialize;
use tf_plugin_api::PluginError;
use url::Url;

use crate::Diag;
use crate::fetch::{
    self, HeaderArg, HeaderList, HeaderView, NON_WILDCARD_NAME, ResponseArg,
    SAFELISTED_RESPONSE_NAMES,
};

/// 条件判定状态：pass | fail | unknown
pub const PASS: &str = "pass";
pub const FAIL: &str = "fail";

/// 阶段状态：skipped | allowed | blocked | undetermined
pub const SKIPPED: &str = "skipped";
pub const ALLOWED: &str = "allowed";
pub const BLOCKED: &str = "blocked";
pub const UNDETERMINED: &str = "undetermined";

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Condition {
    /// i18n 条件名：conditions.<id>
    pub id: &'static str,
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<Diag>,
}

impl Condition {
    fn pass(id: &'static str, detail: Option<String>) -> Self {
        Self {
            id,
            status: PASS,
            detail,
            reason: None,
        }
    }

    fn fail(id: &'static str, detail: Option<String>, reason: Diag) -> Self {
        Self {
            id,
            status: FAIL,
            detail,
            reason: Some(reason),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage {
    pub status: &'static str,
    pub conditions: Vec<Condition>,
    pub reasons: Vec<Diag>,
}

impl Stage {
    fn skipped(reasons: Vec<Diag>) -> Self {
        Self {
            status: SKIPPED,
            conditions: Vec::new(),
            reasons,
        }
    }

    fn of(conditions: Vec<Condition>, extra_reasons: Vec<Diag>) -> Self {
        let mut reasons = extra_reasons;
        reasons.extend(conditions.iter().filter_map(|c| c.reason.clone()));
        let status = if conditions.iter().any(|c| c.status == FAIL) {
            BLOCKED
        } else if conditions.iter().any(|c| c.status == "unknown") {
            UNDETERMINED
        } else {
            ALLOWED
        };
        Self {
            status,
            conditions,
            reasons,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionsDraft {
    pub url: String,
    pub headers: Vec<HeaderView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestView {
    pub origin: String,
    pub target: String,
    pub method: String,
    pub credentials: &'static str,
    pub same_origin: bool,
    pub preflight_needed: bool,
    /// 触发预检的请求头名称（小写、去重、保持出现顺序）
    pub unsafe_header_names: Vec<String>,
    /// 浏览器禁止脚本设置的头（小写、去重）
    pub forbidden_header_names: Vec<String>,
    pub options: OptionsDraft,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub status: &'static str,
    pub conditions: Vec<Condition>,
    pub reasons: Vec<Diag>,
    /// CORS 过滤后脚本可读的响应头名称
    pub readable: Vec<String>,
    /// 被暴露但仍读不到的响应头（forbidden response header）
    pub suppressed: Vec<Diag>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub request: RequestView,
    pub preflight: Stage,
    pub reading: Reading,
    /// allowed | blocked | undetermined
    pub overall: &'static str,
    /// same_origin | forbidden_header | unsupported_scheme | no_preflight |
    /// preflight_ok | preflight_failed | sent_but_unreadable | undetermined
    pub outcome: &'static str,
    pub diagnostics: Vec<Diag>,
}

/// 凭据模式（fetch credentials）
#[derive(Clone, Copy, PartialEq)]
pub enum Credentials {
    Omit,
    SameOrigin,
    Include,
}

impl Credentials {
    fn parse(text: &str) -> Result<Self, PluginError> {
        match text.trim() {
            "" | "same-origin" | "sameOrigin" => Ok(Self::SameOrigin),
            "omit" => Ok(Self::Omit),
            "include" => Ok(Self::Include),
            other => Err(PluginError::new("cors.invalid_credentials").with("value", other)),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Omit => "omit",
            Self::SameOrigin => "same-origin",
            Self::Include => "include",
        }
    }
}

/// 页面 origin：支持字面 `null`（不透明来源）；file: 等同样序列化为 null
fn parse_origin(input: &str, code: &str) -> Result<(String, Option<url::Origin>), PluginError> {
    let text = input.trim();
    if text.eq_ignore_ascii_case("null") {
        return Ok(("null".to_owned(), None));
    }
    let url = Url::parse(text).map_err(|e| PluginError::new(code).with("detail", e.to_string()))?;
    let origin = url.origin();
    let serialized = serialize_origin(&origin);
    Ok((serialized, Some(origin)))
}

/// url crate 的 Origin 未实现 Display：按规范序列化（默认端口省略、不透明来源为 null）
fn serialize_origin(origin: &url::Origin) -> String {
    match origin {
        url::Origin::Tuple(scheme, host, port) => {
            let default = matches!(
                (scheme.as_str(), *port),
                ("http", 80) | ("https", 443) | ("ws", 80) | ("wss", 443) | ("ftp", 21)
            );
            if default {
                format!("{scheme}://{host}")
            } else {
                format!("{scheme}://{host}:{port}")
            }
        }
        url::Origin::Opaque(_) => "null".to_owned(),
    }
}

/// 判定 Access-Control-Allow-Origin（预检与实际响应共用，Fetch §3.2.3 / §3.2.2）
fn check_acao(headers: &HeaderList, origin: &str, include: bool, phase: &str) -> Condition {
    let values = headers.get("access-control-allow-origin");
    if values.is_empty() {
        return Condition::fail(
            "acao",
            None,
            Diag::new("cors.missing_acao").param("phase", phase),
        );
    }
    if values.len() > 1 {
        return Condition::fail(
            "acao",
            Some(values.join(", ")),
            Diag::new("cors.duplicate_acao").param("value", values.join(", ")),
        );
    }
    let value = values[0].trim();
    if value == "*" {
        return if include {
            Condition::fail(
                "acao",
                Some(value.to_owned()),
                Diag::new("cors.acao_wildcard_credentials"),
            )
        } else {
            Condition::pass("acao", Some("*".to_owned()))
        };
    }
    // 多个 origin 拼成一个值不是合法的单一来源，不能当作允许
    if value.contains(',') {
        return Condition::fail(
            "acao",
            Some(value.to_owned()),
            Diag::new("cors.acao_invalid").param("value", value),
        );
    }
    if value == origin {
        Condition::pass("acao", Some(value.to_owned()))
    } else {
        Condition::fail(
            "acao",
            Some(value.to_owned()),
            Diag::new("cors.acao_mismatch")
                .param("expected", origin)
                .param("value", value),
        )
    }
}

/// 判定 Access-Control-Allow-Credentials（仅凭据模式 include 需要）
fn check_acac(headers: &HeaderList) -> Condition {
    let values = headers.get("access-control-allow-credentials");
    if values.is_empty() {
        return Condition::fail("acac", None, Diag::new("cors.acac_missing"));
    }
    if values.len() == 1 && values[0].trim() == "true" {
        Condition::pass("acac", Some("true".to_owned()))
    } else {
        Condition::fail(
            "acac",
            Some(values.join(", ")),
            Diag::new("cors.acac_invalid").param("value", values.join(", ")),
        )
    }
}

/// 判定 Access-Control-Allow-Methods（Fetch §3.2.3-5）
fn check_acam(headers: &HeaderList, method: &str, include: bool) -> Condition {
    let values = headers.get("access-control-allow-methods");
    if values.is_empty() {
        return Condition::fail(
            "acam",
            None,
            Diag::new("cors.acam_missing").param("method", method),
        );
    }
    let joined = values.join(", ");
    let methods: Vec<String> = values
        .iter()
        .flat_map(|v| v.split(','))
        .map(|m| m.trim().to_owned())
        .filter(|m| !m.is_empty())
        .collect();
    // `*` 只在无凭据时是方法通配符
    if !include && methods.iter().any(|m| m == "*") {
        return Condition::pass("acam", Some("*".to_owned()));
    }
    if methods.iter().any(|m| m == method) {
        Condition::pass("acam", Some(joined))
    } else {
        Condition::fail(
            "acam",
            Some(joined.clone()),
            Diag::new("cors.acam_mismatch")
                .param("method", method)
                .param("values", joined),
        )
    }
}

/// 判定 Access-Control-Allow-Headers（Fetch §3.2.3-6）
fn check_acah(headers: &HeaderList, unsafe_names: &[String], include: bool) -> Condition {
    let detail_names = unsafe_names.join(", ");
    let values = headers.get("access-control-allow-headers");
    if values.is_empty() {
        return Condition::fail(
            "acah",
            None,
            Diag::new("cors.acah_missing").param("names", detail_names),
        );
    }
    let joined = values.join(", ");
    let allowed: Vec<String> = values
        .iter()
        .flat_map(|v| v.split(','))
        .map(|h| h.trim().to_ascii_lowercase())
        .filter(|h| !h.is_empty())
        .collect();
    // `*` 只在无凭据时是请求头通配符
    let wildcard = !include && allowed.iter().any(|h| h == "*");
    let mut missing = Vec::new();
    let mut authorization_blocked = false;
    for name in unsafe_names {
        // Authorization 是 CORS-non-wildcard request-header name：通配符永远不覆盖它
        if name == NON_WILDCARD_NAME {
            if !allowed.iter().any(|h| h == NON_WILDCARD_NAME) {
                authorization_blocked = true;
            }
            continue;
        }
        if !wildcard && !allowed.iter().any(|h| h == name) {
            missing.push(name.clone());
        }
    }
    if authorization_blocked {
        return Condition::fail(
            "acah",
            Some(joined),
            Diag::new("cors.acah_authorization_wildcard"),
        );
    }
    if !missing.is_empty() {
        return Condition::fail(
            "acah",
            Some(joined),
            Diag::new("cors.acah_missing_names").param("names", missing.join(", ")),
        );
    }
    Condition::pass("acah", Some(if wildcard { "*".to_owned() } else { joined }))
}

/// 预检响应状态必须是 ok status（200-299）
fn check_preflight_status(status: Option<i64>) -> Condition {
    match status {
        Some(code) if (200..=299).contains(&code) => {
            Condition::pass("status", Some(code.to_string()))
        }
        Some(code) => Condition::fail(
            "status",
            Some(code.to_string()),
            Diag::new("cors.preflight_status").param("status", code),
        ),
        // 状态缺失（只填了头部）时无法判定
        None => Condition {
            id: "status",
            status: "unknown",
            detail: None,
            reason: None,
        },
    }
}

/// 实际响应经 CORS 过滤后脚本可读的头部（Fetch §2.2.7 + Access-Control-Expose-Headers）
fn readable_headers(actual: &HeaderList, include: bool) -> (Vec<String>, Vec<Diag>) {
    let mut diags = Vec::new();
    let expose_values = actual.get("access-control-expose-headers");
    let exposed: Vec<String> = expose_values
        .iter()
        .flat_map(|v| v.split(','))
        .map(|h| h.trim().to_ascii_lowercase())
        .filter(|h| !h.is_empty())
        .collect();
    // `*` 只在无凭据时是可读头通配符；凭据模式下它不生效
    let wildcard = !include && exposed.iter().any(|h| h == "*");
    if include
        && expose_values
            .iter()
            .any(|v| v.split(',').any(|h| h.trim() == "*"))
    {
        diags.push(Diag::new("cors.expose_wildcard_credentials"));
    }
    let mut readable: Vec<String> = actual
        .names()
        .into_iter()
        .filter(|name| {
            !fetch::is_forbidden_response_name(name)
                && (wildcard
                    || SAFELISTED_RESPONSE_NAMES.contains(&name.as_str())
                    || exposed.iter().any(|h| h == name))
        })
        .collect();
    // forbidden response header 即使被显式暴露也读不到
    let mut suppressed = Vec::new();
    for name in actual.names() {
        if fetch::is_forbidden_response_name(&name)
            && (wildcard || exposed.iter().any(|h| h == &name))
        {
            suppressed.push(Diag::new("cors.forbidden_response_name").param("name", name));
        }
    }
    readable.sort();
    readable.dedup();
    (readable, suppressed)
}

pub fn run(
    document_url: &str,
    target_url: &str,
    method: &str,
    credentials_text: &str,
    request_headers: &[HeaderArg],
    preflight: Option<&ResponseArg>,
    actual: Option<&ResponseArg>,
) -> Result<Report, PluginError> {
    let credentials = Credentials::parse(credentials_text)?;
    let method = method.trim();
    if method.is_empty() || !fetch::is_header_name_token(method) {
        return Err(PluginError::new("cors.invalid_method").with("value", method));
    }
    let (origin, document) = parse_origin(document_url, "cors.invalid_document")?;
    let target = Url::parse(target_url.trim())
        .map_err(|e| PluginError::new("cors.invalid_target").with("detail", e.to_string()))?;
    let request = fetch::build_headers(request_headers)?;
    fetch::validate_status(preflight.and_then(|r| r.status), "cors.invalid_status")?;
    fetch::validate_status(actual.and_then(|r| r.status), "cors.invalid_status")?;

    let target_origin = target.origin();
    // 不透明来源（file: 等）之间无法建立可信的同源比较，一律按跨源处理
    let same_origin = match &document {
        Some(doc) => *doc == target_origin,
        None => false,
    };
    let scheme_ok = matches!(target.scheme(), "http" | "https");

    let mut diagnostics = Vec::new();
    let forbidden = request.forbidden_names();
    for name in &forbidden {
        diagnostics.push(Diag::new("cors.forbidden_header").param("name", name.as_str()));
    }
    if !scheme_ok {
        diagnostics
            .push(Diag::new("cors.unsupported_scheme").param("scheme", target.scheme().to_owned()));
    }

    let unsafe_names = request.unsafe_names();
    let preflight_needed = !same_origin
        && scheme_ok
        && (!fetch::is_safelisted_method(method) || !unsafe_names.is_empty());

    // OPTIONS 预检草稿（浏览器自动发出的请求；Host/UA 等由浏览器管理，不列出）
    let mut draft = vec![
        HeaderView::new("Origin", &origin),
        HeaderView::new("Access-Control-Request-Method", method),
    ];
    if !unsafe_names.is_empty() {
        draft.push(HeaderView::new(
            "Access-Control-Request-Headers",
            &unsafe_names.join(", "),
        ));
    }
    let request_view = RequestView {
        origin: origin.clone(),
        target: target.to_string(),
        method: method.to_owned(),
        credentials: credentials.label(),
        same_origin,
        preflight_needed,
        unsafe_header_names: unsafe_names.clone(),
        forbidden_header_names: forbidden,
        options: OptionsDraft {
            url: target.to_string(),
            headers: draft,
        },
    };

    let include = credentials == Credentials::Include;

    // ---- 阶段 0：请求能否发出 ----
    if !request_view.forbidden_header_names.is_empty() {
        // 浏览器会拒绝设置这些头，请求根本不会发出
        return Ok(Report {
            request: request_view,
            preflight: Stage::skipped(Vec::new()),
            reading: Reading {
                status: SKIPPED,
                conditions: Vec::new(),
                reasons: vec![Diag::new("cors.forbidden_header_request")],
                readable: Vec::new(),
                suppressed: Vec::new(),
            },
            overall: UNDETERMINED,
            outcome: "forbidden_header",
            diagnostics,
        });
    }
    if !scheme_ok {
        return Ok(Report {
            request: request_view,
            preflight: Stage::skipped(Vec::new()),
            reading: Reading {
                status: SKIPPED,
                conditions: Vec::new(),
                reasons: Vec::new(),
                readable: Vec::new(),
                suppressed: Vec::new(),
            },
            overall: UNDETERMINED,
            outcome: "unsupported_scheme",
            diagnostics,
        });
    }
    if same_origin {
        diagnostics.push(Diag::new("cors.same_origin"));
        return Ok(Report {
            request: request_view,
            preflight: Stage::skipped(Vec::new()),
            reading: Reading {
                status: SKIPPED,
                conditions: Vec::new(),
                reasons: Vec::new(),
                readable: Vec::new(),
                suppressed: Vec::new(),
            },
            overall: ALLOWED,
            outcome: "same_origin",
            diagnostics,
        });
    }

    // ---- 阶段 1：预检 ----
    let (preflight_stage, preflight_allowed) = if !preflight_needed {
        (Stage::skipped(Vec::new()), true)
    } else if let Some(response) = preflight {
        let headers = fetch::build_headers(&response.headers)?;
        let mut conditions = vec![check_preflight_status(response.status)];
        conditions.push(check_acao(&headers, &origin, include, "preflight"));
        if include {
            conditions.push(check_acac(&headers));
        }
        if !fetch::is_safelisted_method(method) {
            conditions.push(check_acam(&headers, method, include));
        }
        if !unsafe_names.is_empty() {
            conditions.push(check_acah(&headers, &unsafe_names, include));
        }
        let stage = Stage::of(conditions, Vec::new());
        let allowed = stage.status == ALLOWED;
        (stage, allowed)
    } else {
        // 未填预检响应：尚无法判断，不伪造结论
        let stage = Stage {
            status: UNDETERMINED,
            conditions: Vec::new(),
            reasons: vec![Diag::new("cors.preflight_missing")],
        };
        (stage, false)
    };

    // ---- 阶段 2：实际请求与读取 ----
    let reading = if !preflight_allowed {
        // 只有预检明确失败才确定不发实际请求；预检未知时保持未知
        let reasons = if preflight_stage.status == BLOCKED {
            vec![Diag::new("cors.preflight_failed_no_request")]
        } else {
            Vec::new()
        };
        Reading {
            status: SKIPPED,
            conditions: Vec::new(),
            reasons,
            readable: Vec::new(),
            suppressed: Vec::new(),
        }
    } else {
        let Some(response) = actual else {
            return Ok(Report {
                request: request_view,
                preflight: preflight_stage,
                reading: Reading {
                    status: UNDETERMINED,
                    conditions: Vec::new(),
                    reasons: vec![Diag::new("cors.actual_missing")],
                    readable: Vec::new(),
                    suppressed: Vec::new(),
                },
                overall: UNDETERMINED,
                outcome: "undetermined",
                diagnostics,
            });
        };
        let headers = fetch::build_headers(&response.headers)?;
        let mut conditions = vec![check_acao(&headers, &origin, include, "actual")];
        if include {
            conditions.push(check_acac(&headers));
        }
        let allowed = conditions.iter().all(|c| c.status == PASS);
        let (readable, suppressed) = if allowed {
            readable_headers(&headers, include)
        } else {
            (Vec::new(), Vec::new())
        };
        let blocked = conditions.iter().any(|c| c.status == FAIL);
        Reading {
            status: if blocked { BLOCKED } else { ALLOWED },
            conditions,
            reasons: Vec::new(),
            readable,
            suppressed,
        }
    };

    let (overall, outcome) = if preflight_stage.status == BLOCKED {
        (BLOCKED, "preflight_failed")
    } else if preflight_stage.status == UNDETERMINED || reading.status == UNDETERMINED {
        (UNDETERMINED, "undetermined")
    } else if reading.status == BLOCKED {
        (BLOCKED, "sent_but_unreadable")
    } else if preflight_needed {
        (ALLOWED, "preflight_ok")
    } else {
        (ALLOWED, "no_preflight")
    };

    Ok(Report {
        request: request_view,
        preflight: preflight_stage,
        reading,
        overall,
        outcome,
        diagnostics,
    })
}

/// Markdown 报告：结论 + 请求 + OPTIONS 草稿 + 逐阶段条件 + 可读头
pub fn markdown(report: &Report) -> String {
    let mut out = String::new();
    out.push_str("# CORS Preflight Inspection\n\n");
    out.push_str(&format!(
        "- Outcome: **{}** ({})\n",
        report.overall, report.outcome
    ));
    out.push_str(&format!("- Origin: `{}`\n", report.request.origin));
    out.push_str(&format!("- Target: `{}`\n", report.request.target));
    out.push_str(&format!(
        "- Method / credentials: `{}` / `{}`\n",
        report.request.method, report.request.credentials
    ));
    out.push_str(&format!(
        "- Preflight: {}\n\n",
        if report.request.preflight_needed {
            "needed"
        } else {
            "not needed"
        }
    ));
    out.push_str("## OPTIONS draft\n\n```http\nOPTIONS ");
    out.push_str(&report.request.options.url);
    out.push('\n');
    for header in &report.request.options.headers {
        out.push_str(&format!("{}: {}\n", header.name, header.value));
    }
    out.push_str("```\n\n");
    if !report.preflight.conditions.is_empty() {
        out.push_str("## Preflight checks\n\n");
        for condition in &report.preflight.conditions {
            out.push_str(&format!(
                "- {}: {}{}\n",
                condition.id,
                condition.status,
                condition
                    .detail
                    .as_ref()
                    .map(|d| format!(" (`{d}`)"))
                    .unwrap_or_default(),
            ));
        }
        out.push('\n');
    }
    if !report.reading.conditions.is_empty() || !report.reading.readable.is_empty() {
        out.push_str("## Response reading\n\n");
        for condition in &report.reading.conditions {
            out.push_str(&format!(
                "- {}: {}{}\n",
                condition.id,
                condition.status,
                condition
                    .detail
                    .as_ref()
                    .map(|d| format!(" (`{d}`)"))
                    .unwrap_or_default(),
            ));
        }
        if !report.reading.readable.is_empty() {
            out.push_str(&format!(
                "- Readable headers: {}\n",
                report
                    .reading
                    .readable
                    .iter()
                    .map(|h| format!("`{h}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        out.push('\n');
    }
    if !report.diagnostics.is_empty() {
        out.push_str("## Diagnostics\n\n");
        for diag in &report.diagnostics {
            out.push_str(&format!("- `{}`\n", diag.code));
        }
    }
    out
}

#[cfg(test)]
#[path = "verdict_test.rs"]
mod tests;
