//! 逐资源判定：指令回退链、每条策略独立判定、多策略同时生效取交集、
//! Report-Only 只报告不拦截、strict-dynamic 等依赖执行链的语义给出
//! 「无法判定」而不是假的允许/阻止。

use serde::Serialize;
use tf_plugin_api::PluginError;
use url::Url;

use crate::Diag;
use crate::policy::{
    self, Directive, Expr, MatchCtx, Origin, Policy, UrlParts, parse_loose_url, parse_policies,
    url_parts,
};

/// 资源类型 → 指令回退链（第一个存在的指令生效）
pub const RESOURCE_CHAINS: &[(&str, &[&str])] = &[
    ("script", &["script-src", "default-src"]),
    ("style", &["style-src", "default-src"]),
    ("image", &["img-src", "default-src"]),
    ("font", &["font-src", "default-src"]),
    ("connect", &["connect-src", "default-src"]),
    ("frame", &["frame-src", "child-src", "default-src"]),
    ("worker", &["worker-src", "child-src", "default-src"]),
    ("media", &["media-src", "default-src"]),
    ("object", &["object-src", "default-src"]),
    ("manifest", &["manifest-src", "default-src"]),
];

fn chain_for(kind: &str) -> Option<&'static [&'static str]> {
    RESOURCE_CHAINS
        .iter()
        .find(|(name, _)| *name == kind)
        .map(|(_, chain)| *chain)
}

#[derive(Serialize)]
pub struct DirectiveView {
    pub name: String,
    pub values: Vec<String>,
}

#[derive(Serialize)]
pub struct PolicyView {
    pub index: usize,
    pub report_only: bool,
    pub raw: String,
    pub directives: Vec<DirectiveView>,
    pub diagnostics: Vec<Diag>,
}

#[derive(Serialize)]
pub struct PolicyVerdict {
    pub index: usize,
    pub report_only: bool,
    /// allowed | blocked | undetermined | unrestricted
    pub status: &'static str,
    pub directive: Option<String>,
    pub matched: Option<String>,
}

#[derive(Serialize)]
pub struct ResourceVerdict {
    pub index: usize,
    pub kind: String,
    pub url: Option<String>,
    /// allowed | blocked | undetermined | unrestricted
    pub status: &'static str,
    pub directive: Option<String>,
    pub matched: Option<String>,
    pub policies: Vec<PolicyVerdict>,
    pub reasons: Vec<Diag>,
}

#[derive(Serialize)]
pub struct Report {
    pub document: Option<String>,
    pub policies: Vec<PolicyView>,
    pub resources: Vec<ResourceVerdict>,
    pub diagnostics: Vec<Diag>,
    pub stats: Stats,
}

#[derive(Serialize)]
pub struct Stats {
    pub allowed: usize,
    pub blocked: usize,
    pub undetermined: usize,
    pub unrestricted: usize,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceInput {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub nonce: Option<String>,
    #[serde(default)]
    pub hash: Option<String>,
}

/// 单条策略下的判定：找到回退链里第一个存在的指令后评估其全部来源表达式
fn evaluate(
    policy: &Policy,
    chain: &[&str],
    document: Option<&Origin>,
    url: &UrlParts,
    nonce: Option<&str>,
    hash: Option<&str>,
) -> PolicyVerdict {
    for name in chain {
        let Some(directive) = policy.directive(name) else {
            continue;
        };
        let ctx = MatchCtx {
            document,
            url: Some(url),
            nonce,
            hash,
        };
        if directive
            .values
            .iter()
            .any(|v| v.eq_ignore_ascii_case("'none'"))
        {
            return PolicyVerdict {
                index: 0,
                report_only: policy.report_only,
                status: "blocked",
                directive: Some(directive.name.clone()),
                matched: Some("'none'".to_owned()),
            };
        }
        let mut undetermined = false;
        for value in &directive.values {
            let Some(expr) = policy::parse_expr(value) else {
                continue;
            };
            // strict-dynamic 使主机来源对脚本失效，静态无法判定执行链放行
            if policy.strict_dynamic_script
                && directive.name == "script-src"
                && matches!(expr, Expr::Host { .. })
            {
                continue;
            }
            match policy::matches(&expr, &ctx) {
                Ok(true) => {
                    return PolicyVerdict {
                        index: 0,
                        report_only: policy.report_only,
                        status: "allowed",
                        directive: Some(directive.name.clone()),
                        matched: Some(value.clone()),
                    };
                }
                Ok(false) => {}
                Err(()) => undetermined = true,
            }
        }
        if undetermined {
            return undetermined_verdict(policy, directive);
        }
        if policy.strict_dynamic_script && directive.name == "script-src" {
            // strict-dynamic 会让主机来源对脚本失效，是否放行取决于执行链
            return undetermined_verdict(policy, directive);
        }
        return PolicyVerdict {
            index: 0,
            report_only: policy.report_only,
            status: "blocked",
            directive: Some(directive.name.clone()),
            matched: None,
        };
    }
    PolicyVerdict {
        index: 0,
        report_only: policy.report_only,
        status: "unrestricted",
        directive: None,
        matched: None,
    }
}

fn undetermined_verdict(policy: &Policy, directive: &Directive) -> PolicyVerdict {
    PolicyVerdict {
        index: 0,
        report_only: policy.report_only,
        status: "undetermined",
        directive: Some(directive.name.clone()),
        matched: None,
    }
}

pub fn run(
    policies_text: &str,
    document_url: Option<&str>,
    resources: &[ResourceInput],
) -> Result<Report, PluginError> {
    let policies = parse_policies(policies_text)?;
    let document = match document_url {
        Some(text) if !text.trim().is_empty() => {
            let url = parse_loose_url(text)?;
            Some((url.clone(), policy::origin(&url)))
        }
        _ => None,
    };
    let document_url_value = document.as_ref().map(|(url, _)| url.to_string());

    let mut policy_views = Vec::new();
    let mut resource_verdicts = Vec::new();
    let mut stats = Stats {
        allowed: 0,
        blocked: 0,
        undetermined: 0,
        unrestricted: 0,
    };

    let mut diagnostics = Vec::new();
    // 'self' 出现在任何策略里但文档 URL 缺失时提示
    let uses_self = policies.iter().any(|policy| {
        policy.directives.iter().any(|directive| {
            directive
                .values
                .iter()
                .any(|value| matches!(policy::parse_expr(value), Some(Expr::SelfOrigin)))
        })
    });
    if uses_self && document.is_none() {
        diagnostics.push(Diag::new("csp.self_without_document"));
    }

    for (policy_index, policy) in policies.iter().enumerate() {
        policy_views.push(PolicyView {
            index: policy_index,
            report_only: policy.report_only,
            raw: policy.raw.clone(),
            directives: policy
                .directives
                .iter()
                .map(|directive| DirectiveView {
                    name: directive.name.clone(),
                    values: directive.values.clone(),
                })
                .collect(),
            diagnostics: policy.diagnostics.clone(),
        });
    }

    for (index, resource) in resources.iter().enumerate() {
        if resource_verdicts.len() >= policy::MAX_RESOURCES {
            diagnostics
                .push(Diag::new("csp.too_many_resources").param("max", policy::MAX_RESOURCES));
            break;
        }
        if !matches!(
            resource.kind.as_str(),
            "script"
                | "style"
                | "image"
                | "font"
                | "connect"
                | "frame"
                | "worker"
                | "media"
                | "object"
                | "manifest"
        ) {
            resource_verdicts.push(ResourceVerdict {
                index,
                kind: resource.kind.clone(),
                url: resource.url.clone(),
                status: "unrestricted",
                directive: None,
                matched: None,
                policies: Vec::new(),
                reasons: vec![Diag::new("csp.unknown_type").param("type", resource.kind.clone())],
            });
            stats.unrestricted += 1;
            continue;
        }
        let chain = chain_for(&resource.kind).expect("known kind");
        let url: Option<Url> = resource.url.as_deref().map(parse_loose_url).transpose()?;
        let nonce = resource.nonce.as_deref().map(normalize_nonce);
        let hash = resource.hash.as_deref().map(normalize_hash);
        let mut per_policy = Vec::new();
        let mut reasons = Vec::new();
        let mut blocked = false;
        let mut undetermined = false;
        let mut restricted = false;
        let mut first_blocked: Option<(Option<String>, Option<String>)> = None;
        let mut first_restricted: Option<(Option<String>, Option<String>)> = None;
        for (policy_index, policy) in policies.iter().enumerate() {
            // 同一策略内的 upgrade-insecure-requests：把 http 资源升级为 https 再匹配
            let upgraded = policy.upgrade && url.as_ref().is_some_and(|u| u.scheme() == "http");
            let parts: UrlParts = match (&url, upgraded) {
                (Some(url), true) => upgrade_parts(&url_parts(url)),
                (Some(url), false) => url_parts(url),
                (None, _) => UrlParts {
                    scheme: String::new(),
                    host: String::new(),
                    port: 0,
                    path: String::new(),
                },
            };
            let document_origin = document.as_ref().map(|(_, origin)| origin);
            let nonce_value = nonce.as_deref();
            let hash_value = hash.as_deref();
            let mut verdict = if url.is_none() && nonce.is_none() && hash.is_none() {
                // 既没有 URL 也没有内联元数据：静态无法判定
                match chain.iter().find_map(|name| policy.directive(name)) {
                    Some(directive) => undetermined_verdict(policy, directive),
                    None => PolicyVerdict {
                        index: 0,
                        report_only: policy.report_only,
                        status: "unrestricted",
                        directive: None,
                        matched: None,
                    },
                }
            } else {
                evaluate(
                    policy,
                    chain,
                    document_origin,
                    &parts,
                    nonce_value,
                    hash_value,
                )
            };
            verdict.index = policy_index;
            if policy.report_only {
                if verdict.status == "blocked" {
                    reasons.push(
                        Diag::new("csp.report_only_would_block").param("index", verdict.index),
                    );
                }
                // Report-Only 只报告，不影响总状态
            } else {
                restricted = true;
                match verdict.status {
                    "blocked" => {
                        blocked = true;
                        if first_blocked.is_none() {
                            first_blocked =
                                Some((verdict.directive.clone(), verdict.matched.clone()));
                        }
                    }
                    "undetermined" => {
                        undetermined = true;
                        if first_restricted.is_none() {
                            first_restricted =
                                Some((verdict.directive.clone(), verdict.matched.clone()));
                        }
                    }
                    "allowed" if first_restricted.is_none() => {
                        first_restricted =
                            Some((verdict.directive.clone(), verdict.matched.clone()));
                    }
                    _ => {}
                }
            }
            per_policy.push(verdict);
        }
        let status = if blocked {
            "blocked"
        } else if undetermined {
            reasons.push(Diag::new("csp.undetermined"));
            "undetermined"
        } else if !restricted {
            "unrestricted"
        } else {
            "allowed"
        };
        match status {
            "allowed" => stats.allowed += 1,
            "blocked" => stats.blocked += 1,
            "undetermined" => stats.undetermined += 1,
            _ => stats.unrestricted += 1,
        }
        resource_verdicts.push(ResourceVerdict {
            index,
            kind: resource.kind.clone(),
            url: resource.url.clone(),
            status,
            directive: first_blocked
                .as_ref()
                .or(first_restricted.as_ref())
                .and_then(|(directive, _)| directive.clone()),
            matched: first_blocked
                .as_ref()
                .or(first_restricted.as_ref())
                .and_then(|(_, matched)| matched.clone()),
            policies: per_policy,
            reasons,
        });
    }

    Ok(Report {
        document: document_url_value,
        policies: policy_views,
        resources: resource_verdicts,
        diagnostics,
        stats,
    })
}

fn upgrade_parts(parts: &UrlParts) -> UrlParts {
    let port = if parts.port == 80 { 443 } else { parts.port };
    UrlParts {
        scheme: "https".to_owned(),
        host: parts.host.clone(),
        port,
        path: parts.path.clone(),
    }
}

fn normalize_nonce(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.starts_with('\'') {
        trimmed.to_ascii_lowercase()
    } else {
        format!("'nonce-{}'", trimmed.to_ascii_lowercase())
    }
}

fn normalize_hash(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.starts_with('\'') {
        trimmed.to_ascii_lowercase()
    } else {
        format!("'sha256-{}'", trimmed.to_ascii_lowercase())
    }
}

/// Markdown 报告：判定表 + 策略清单
pub fn markdown(report: &Report) -> String {
    let mut out = String::new();
    out.push_str("# CSP Inspection\n\n");
    if let Some(document) = &report.document {
        out.push_str(&format!("- Document: `{document}`\n"));
    }
    out.push_str(&format!(
        "- Resources: {} (allowed {}, blocked {}, undetermined {}, unrestricted {})\n\n",
        report.resources.len(),
        report.stats.allowed,
        report.stats.blocked,
        report.stats.undetermined,
        report.stats.unrestricted,
    ));
    for policy in &report.policies {
        out.push_str(&format!(
            "## Policy {}{}\n\n```text\n{}\n```\n\n",
            policy.index + 1,
            if policy.report_only {
                " (report-only)"
            } else {
                ""
            },
            policy.raw,
        ));
    }
    out.push_str("## Verdicts\n\n");
    for resource in &report.resources {
        out.push_str(&format!(
            "- `{}` {} — **{}**",
            resource.kind,
            resource.url.as_deref().unwrap_or("(inline)"),
            resource.status,
        ));
        if let Some(directive) = &resource.directive {
            out.push_str(&format!(" (via `{directive}`"));
            if let Some(matched) = &resource.matched {
                out.push_str(&format!(", matched `{matched}`"));
            }
            out.push(')');
        }
        out.push('\n');
        for reason in &resource.reasons {
            out.push_str(&format!("  - `{}`\n", reason.code));
        }
    }
    out
}

#[cfg(test)]
#[path = "verdict_test.rs"]
mod tests;
