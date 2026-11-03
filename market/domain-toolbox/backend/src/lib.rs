//! 域名工具箱插件后端（完全离线）：候选域名批量生成、常见场景 DNS 记录配置生成、
//! SPF 与 DMARC 记录语法校验。RDAP 查询与资产管理不在首版范围。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct DomainToolbox {
    manifest: Manifest,
}

impl Default for DomainToolbox {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for DomainToolbox {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "candidates" => to_value(candidates(parse_args(args)?)?),
            "dns" => to_value(dns(parse_args(args)?)?),
            "validate_spf" => to_value(validate_spf_record(parse_args(args)?)?),
            "validate_dmarc" => to_value(validate_dmarc(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(DomainToolbox);

const PRICE_TLDS: &[(&str, &str)] = &[
    ("com", "standard"),
    ("net", "standard"),
    ("org", "standard"),
    ("io", "premium"),
    ("dev", "premium"),
    ("app", "premium"),
    ("ai", "expensive"),
    ("co", "standard"),
    ("cn", "standard"),
];

fn is_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= 63
        && label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && !label.starts_with('-')
        && !label.ends_with('-')
}

fn valid_domain(domain: &str) -> bool {
    let domain = domain.trim().trim_end_matches('.').to_lowercase();
    let parts: Vec<&str> = domain.split('.').collect();
    parts.len() >= 2 && parts.iter().all(|p| is_label(p))
}

/// 候选生成：brands × tlds，附连字符与 get 前缀变体
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateArgs {
    /// 品牌词/关键词（1–8 个）
    pub brands: Vec<String>,
    /// 目标 TLD（不含点；缺省用内置常见集合）
    #[serde(default)]
    pub tlds: Vec<String>,
    /// 是否生成连字符组合（brand-get）
    #[serde(default)]
    pub hyphens: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub domain: String,
    /// tld 价位分组键 prices.<key>
    pub price: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateOutput {
    pub candidates: Vec<Candidate>,
    pub total: usize,
}

pub fn candidates(args: CandidateArgs) -> PluginResult<CandidateOutput> {
    if args.brands.is_empty() || args.brands.len() > 8 {
        return Err(PluginError::new("domain.bad_brands"));
    }
    let brands: Vec<String> = args
        .brands
        .iter()
        .map(|b| b.trim().to_lowercase())
        .filter(|b| is_label(b))
        .collect();
    if brands.is_empty() {
        return Err(PluginError::new("domain.bad_brands"));
    }
    let tlds: Vec<String> = if args.tlds.is_empty() {
        PRICE_TLDS.iter().map(|(t, _)| t.to_string()).collect()
    } else {
        args.tlds
            .iter()
            .map(|t| t.trim().trim_start_matches('.').to_lowercase())
            .filter(|t| is_label(t))
            .collect()
    };
    if tlds.is_empty() || tlds.len() > 20 {
        return Err(PluginError::new("domain.bad_tlds"));
    }
    let mut names: Vec<String> = Vec::new();
    for brand in &brands {
        names.push(brand.clone());
        if args.hyphens {
            for other in &brands {
                if other != brand {
                    names.push(format!("{brand}-{other}"));
                }
            }
            names.push(format!("{brand}-get"));
        }
    }
    let mut candidates = Vec::new();
    for name in &names {
        for tld in &tlds {
            let domain = format!("{name}.{tld}");
            if !valid_domain(&domain) {
                continue;
            }
            let price = PRICE_TLDS
                .iter()
                .find(|(t, _)| t == tld)
                .map(|(_, p)| *p)
                .unwrap_or("standard");
            candidates.push(Candidate { domain, price });
        }
    }
    if candidates.is_empty() {
        return Err(PluginError::new("domain.no_candidates"));
    }
    let total = candidates.len();
    Ok(CandidateOutput { candidates, total })
}

/// DNS 配置生成：website（A/CNAME）、email（MX+SPF+DKIM+DMARC）、cdn（CNAME）
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsArgs {
    /// website / email / cdn
    pub scenario: String,
    /// 根域名
    pub domain: String,
    /// website：服务器 IPv4
    #[serde(default)]
    pub ip: Option<String>,
    /// website/cdn：目标主机（如 shop.example.com 或 cdn.example.net）
    #[serde(default)]
    pub target: Option<String>,
    /// email：邮件服务器主机（默认 mail.<domain>）
    #[serde(default)]
    pub mail_host: Option<String>,
    /// email：DKIM 选择器（默认 default）
    #[serde(default)]
    pub dkim_selector: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsRecord {
    #[serde(rename = "type")]
    pub record_type: &'static str,
    pub name: String,
    pub value: String,
    pub ttl: u32,
    /// 记录用途键（dns_purpose.<key>）
    pub purpose: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsOutput {
    pub scenario: String,
    pub domain: String,
    pub records: Vec<DnsRecord>,
}

fn valid_ip(ip: &str) -> bool {
    let parts: Vec<&str> = ip.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|p| {
            p.chars().all(|c| c.is_ascii_digit())
                && !p.is_empty()
                && p.len() <= 3
                && p.parse::<u16>().is_ok_and(|n| n <= 255)
                && (*p == "0" || !p.starts_with('0'))
        })
}

pub fn dns(args: DnsArgs) -> PluginResult<DnsOutput> {
    let domain = args.domain.trim().trim_end_matches('.').to_lowercase();
    if !valid_domain(&domain) {
        return Err(PluginError::new("domain.bad_domain").with("domain", domain));
    }
    let mut records = Vec::new();
    match args.scenario.as_str() {
        "website" => {
            if let Some(ip) = args.ip.as_deref().map(str::trim) {
                if !valid_ip(ip) {
                    return Err(PluginError::new("domain.bad_ip").with("ip", ip));
                }
                records.push(DnsRecord {
                    record_type: "A",
                    name: "@".into(),
                    value: ip.to_owned(),
                    ttl: 600,
                    purpose: "website_a",
                });
            }
            if let Some(target) = args.target.as_deref().map(str::trim) {
                let target = target.trim_end_matches('.').to_lowercase();
                if !valid_domain(&target) {
                    return Err(PluginError::new("domain.bad_domain").with("domain", target));
                }
                records.push(DnsRecord {
                    record_type: "CNAME",
                    name: "www".into(),
                    value: format!("{target}."),
                    ttl: 600,
                    purpose: "website_www",
                });
            }
            if records.is_empty() {
                return Err(PluginError::new("domain.missing_dns_param"));
            }
        }
        "cdn" => {
            let target = args
                .target
                .as_deref()
                .map(str::trim)
                .ok_or_else(|| PluginError::new("domain.missing_dns_param"))?;
            let target = target.trim_end_matches('.').to_lowercase();
            if !valid_domain(&target) {
                return Err(PluginError::new("domain.bad_domain").with("domain", target));
            }
            records.push(DnsRecord {
                record_type: "CNAME",
                name: "cdn".into(),
                value: format!("{target}."),
                ttl: 600,
                purpose: "cdn_cname",
            });
        }
        "email" => {
            let mail_host = args
                .mail_host
                .as_deref()
                .map(str::trim)
                .map(|h| h.trim_end_matches('.').to_lowercase())
                .unwrap_or_else(|| format!("mail.{domain}"));
            if !valid_domain(&mail_host) {
                return Err(PluginError::new("domain.bad_domain").with("domain", mail_host));
            }
            let selector = args
                .dkim_selector
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("default");
            records.push(DnsRecord {
                record_type: "A",
                name: mail_host.clone(),
                value: args.ip.clone().unwrap_or_else(|| "0.0.0.0".into()),
                ttl: 600,
                purpose: "email_mx_host",
            });
            records.push(DnsRecord {
                record_type: "MX",
                name: "@".into(),
                value: format!("10 {mail_host}."),
                ttl: 3600,
                purpose: "email_mx",
            });
            records.push(DnsRecord {
                record_type: "TXT",
                name: "@".into(),
                value: format!("v=spf1 a mx include:{mail_host} -all"),
                ttl: 3600,
                purpose: "email_spf",
            });
            records.push(DnsRecord {
                record_type: "TXT",
                name: format!("{selector}._domainkey"),
                value: "v=DKIM1; k=rsa; p=<YOUR_PUBLIC_KEY>".into(),
                ttl: 3600,
                purpose: "email_dkim",
            });
            records.push(DnsRecord {
                record_type: "TXT",
                name: "_dmarc".into(),
                value: format!("v=DMARC1; p=quarantine; rua=mailto:dmarc@{domain}; pct=100"),
                ttl: 3600,
                purpose: "email_dmarc",
            });
        }
        other => return Err(PluginError::new("domain.unknown_scenario").with("scenario", other)),
    }
    Ok(DnsOutput {
        scenario: args.scenario,
        domain,
        records,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordArgs {
    /// SPF/DMARC 记录值（TXT 内容）
    pub record: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpfOutput {
    pub valid: bool,
    /// 全部限定机制（按出现顺序）
    pub mechanisms: Vec<String>,
    /// include/redirect 域名
    pub includes: Vec<String>,
    /// 策略强度：soft（~all）到 hard（-all）
    pub policy: String,
    pub issues: Vec<String>,
}

pub fn validate_spf_record(args: RecordArgs) -> PluginResult<SpfOutput> {
    let record = args.record.trim();
    let mut issues = Vec::new();
    if record.len() > 255 {
        issues.push(format!("spf_too_long:{len}", len = record.len()));
    }
    let Some(rest) = record.strip_prefix("v=spf1") else {
        return Ok(SpfOutput {
            valid: false,
            mechanisms: Vec::new(),
            includes: Vec::new(),
            policy: "none".into(),
            issues: vec!["spf_missing_version".into()],
        });
    };
    let mut mechanisms = Vec::new();
    let mut includes = Vec::new();
    let mut policy = "neutral".to_string();
    let mut has_all = false;
    for token in rest.split_whitespace() {
        if token.starts_with("v=") {
            continue;
        }
        let lower = token.to_lowercase();
        if lower.ends_with("all")
            && (lower == "all"
                || lower.ends_with("+all")
                || lower.ends_with("-all")
                || lower.ends_with("~all")
                || lower.ends_with("?all"))
        {
            has_all = true;
            policy = match lower.as_str() {
                "-all" => "hard".into(),
                "~all" => "soft".into(),
                "?all" => "neutral".into(),
                _ => "pass".into(),
            };
            mechanisms.push(token.to_owned());
            continue;
        }
        if let Some(domain) = lower.strip_prefix("include:") {
            if !valid_domain(domain) {
                issues.push(format!("spf_bad_include:{domain}"));
            } else {
                includes.push(domain.to_owned());
            }
            mechanisms.push(token.to_owned());
            continue;
        }
        if lower.starts_with("redirect=") {
            let domain = lower.trim_start_matches("redirect=");
            if !valid_domain(domain) {
                issues.push(format!("spf_bad_redirect:{domain}"));
            } else {
                includes.push(domain.to_owned());
            }
            mechanisms.push(token.to_owned());
            continue;
        }
        // a / mx / ip4: / ip6: / exists: / ptr
        if lower == "a"
            || lower == "mx"
            || lower.starts_with("a:")
            || lower.starts_with("mx:")
            || lower.starts_with("ip4:")
            || lower.starts_with("ip6:")
            || lower.starts_with("exists:")
            || lower == "ptr"
        {
            mechanisms.push(token.to_owned());
            continue;
        }
        issues.push(format!("spf_unknown_term:{token}"));
    }
    if !has_all && issues.is_empty() {
        issues.push("spf_missing_all".into());
    }
    let valid = issues.is_empty();
    Ok(SpfOutput {
        valid,
        mechanisms,
        includes,
        policy,
        issues,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DmarcOutput {
    pub valid: bool,
    /// none / quarantine / reject
    pub policy: String,
    /// 报告地址（rua）
    pub reports: Vec<String>,
    pub percentage: Option<u32>,
    pub issues: Vec<String>,
}

pub fn validate_dmarc(args: RecordArgs) -> PluginResult<DmarcOutput> {
    let record = args.record.trim();
    let mut issues = Vec::new();
    if !record.starts_with("v=DMARC1") {
        return Ok(DmarcOutput {
            valid: false,
            policy: "none".into(),
            reports: Vec::new(),
            percentage: None,
            issues: vec!["dmarc_missing_version".into()],
        });
    }
    let mut policy = String::new();
    let mut reports = Vec::new();
    let mut percentage = None;
    for tag in record.split(';').map(str::trim).filter(|t| !t.is_empty()) {
        let Some((key, value)) = tag.split_once('=') else {
            issues.push(format!("dmarc_bad_tag:{tag}"));
            continue;
        };
        match key.trim().to_lowercase().as_str() {
            "v" => {}
            "p" => match value.trim().to_lowercase().as_str() {
                "none" | "quarantine" | "reject" => policy = value.trim().to_lowercase(),
                other => issues.push(format!("dmarc_bad_policy:{other}")),
            },
            "rua" => {
                for uri in value.split(',').map(str::trim) {
                    if let Some(mail) = uri.strip_prefix("mailto:") {
                        if mail.contains('@') {
                            reports.push(mail.to_owned());
                        } else {
                            issues.push(format!("dmarc_bad_uri:{uri}"));
                        }
                    } else {
                        issues.push(format!("dmarc_bad_uri:{uri}"));
                    }
                }
            }
            "pct" => match value.trim().parse::<u32>() {
                Ok(n) if n <= 100 => percentage = Some(n),
                _ => issues.push(format!("dmarc_bad_pct:{value}")),
            },
            "sp" | "adkim" | "aspf" | "fo" | "rf" | "ri" => {}
            other => issues.push(format!("dmarc_unknown_tag:{other}")),
        }
    }
    if policy.is_empty() {
        issues.push("dmarc_missing_policy".into());
    }
    let valid = issues.is_empty();
    Ok(DmarcOutput {
        valid,
        policy: if policy.is_empty() {
            "none".into()
        } else {
            policy
        },
        reports,
        percentage,
        issues,
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
