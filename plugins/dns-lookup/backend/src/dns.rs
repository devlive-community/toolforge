use std::net::IpAddr;
use std::str::FromStr;
use std::time::{Duration, Instant};

use hickory_resolver::config::{NameServerConfig, ResolverConfig};
use hickory_resolver::net::NetError;
use hickory_resolver::net::runtime::TokioRuntimeProvider;
use hickory_resolver::proto::rr::{Name, RecordType};
use hickory_resolver::{Resolver, TokioResolver};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

const TIMEOUT: Duration = Duration::from_secs(3);
const MAX_SERVERS: usize = 12;

/// 常用公共 DNS
pub const PUBLIC: &[(&str, &str)] = &[
    ("cloudflare", "1.1.1.1"),
    ("google", "8.8.8.8"),
    ("quad9", "9.9.9.9"),
    ("alidns", "223.5.5.5"),
    ("dnspod", "119.29.29.29"),
    ("dns114", "114.114.114.114"),
];

pub const TYPES: &[&str] = &[
    "A", "AAAA", "CNAME", "MX", "TXT", "NS", "SOA", "SRV", "CAA", "PTR",
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    name: String,
    #[serde(default = "default_type")]
    record_type: String,
    /// `system`、公共 DNS 的 id，或任意 IP 地址
    #[serde(default = "default_servers")]
    servers: Vec<String>,
}

fn default_type() -> String {
    "A".into()
}

fn default_servers() -> Vec<String> {
    vec!["system".into()]
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    pub name: String,
    pub record_type: String,
    pub ttl: u32,
    pub value: String,
    /// 198.18.0.0/15 的地址通常是代理（TUN / fake-ip 模式）返回的占位地址
    pub fake_ip: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerResult {
    pub server: String,
    /// 实际使用的地址（系统解析器为空）
    pub address: Option<String>,
    pub ms: u64,
    pub answers: Vec<Answer>,
    pub error: Option<PluginError>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub query: String,
    pub record_type: String,
    pub results: Vec<ServerResult>,
}

/// RFC 2544 基准测试网段，常被代理软件用作 fake-ip
pub fn is_fake_ip(value: &str) -> bool {
    value
        .parse::<std::net::Ipv4Addr>()
        .is_ok_and(|ip| ip.octets()[0] == 198 && (ip.octets()[1] & 0xfe) == 18)
}

/// 规范化查询名：去掉协议与路径；PTR 查询时把 IP 转为反向域名
pub fn query_name(raw: &str, record_type: RecordType) -> PluginResult<Name> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(PluginError::new("dns.empty"));
    }
    let host = trimmed
        .split("://")
        .last()
        .unwrap_or(trimmed)
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    if let Ok(ip) = host.trim_matches(['[', ']']).parse::<IpAddr>() {
        if record_type == RecordType::PTR {
            return Ok(Name::from(ip));
        }
        return Err(PluginError::new("dns.ip_needs_ptr").with("ip", ip.to_string()));
    }
    // 去掉端口
    let host = host.rsplit_once(':').map(|(h, _)| h).unwrap_or(host);
    let mut name = Name::from_str(host).map_err(|e| {
        PluginError::new("dns.invalid_name")
            .with("name", host)
            .with("detail", e.to_string())
    })?;
    name.set_fqdn(true);
    Ok(name)
}

fn record_type(raw: &str) -> PluginResult<RecordType> {
    let upper = raw.trim().to_ascii_uppercase();
    if !TYPES.contains(&upper.as_str()) {
        return Err(PluginError::new("dns.unsupported_type").with("type", raw));
    }
    RecordType::from_str(&upper)
        .map_err(|_| PluginError::new("dns.unsupported_type").with("type", raw))
}

/// 把服务器标识解析为地址；`system` 返回 None
pub fn server_address(server: &str) -> PluginResult<Option<IpAddr>> {
    if server == "system" {
        return Ok(None);
    }
    if let Some((_, ip)) = PUBLIC.iter().find(|(id, _)| *id == server) {
        return Ok(Some(ip.parse().expect("static ip")));
    }
    server
        .trim()
        .parse::<IpAddr>()
        .map(Some)
        .map_err(|_| PluginError::new("dns.invalid_server").with("server", server))
}

fn error(err: NetError) -> PluginError {
    if err.is_nx_domain() {
        PluginError::new("dns.nx_domain")
    } else if err.is_no_records_found() {
        PluginError::new("dns.no_records")
    } else {
        let detail = err.to_string();
        let code =
            if detail.to_ascii_lowercase().contains("timed out") || detail.contains("timeout") {
                "dns.timeout"
            } else {
                "dns.failed"
            };
        PluginError::new(code).with("detail", detail)
    }
}

fn resolver(address: Option<IpAddr>) -> PluginResult<TokioResolver> {
    let mut builder = match address {
        None => TokioResolver::builder_tokio()
            .map_err(|e| PluginError::new("dns.system_config").with("detail", e.to_string()))?,
        Some(ip) => Resolver::builder_with_config(
            ResolverConfig::from_name_servers(vec![NameServerConfig::udp_and_tcp(ip)]),
            TokioRuntimeProvider::new(),
        ),
    };
    let options = builder.options_mut();
    options.timeout = TIMEOUT;
    options.attempts = 1;
    // 每次查询都实际请求服务器，便于比较各服务器的结果与耗时
    options.cache_size = 0;
    builder
        .build()
        .map_err(|e| PluginError::new("dns.failed").with("detail", e.to_string()))
}

async fn query(server: String, name: Name, kind: RecordType) -> ServerResult {
    let address = match server_address(&server) {
        Ok(address) => address,
        Err(err) => {
            return ServerResult {
                server,
                address: None,
                ms: 0,
                answers: vec![],
                error: Some(err),
            };
        }
    };
    let start = Instant::now();
    let outcome = match resolver(address) {
        Ok(resolver) => resolver.lookup(name, kind).await.map_err(error),
        Err(err) => Err(err),
    };
    let ms = start.elapsed().as_millis() as u64;
    let (mut answers, error) = match outcome {
        Ok(lookup) => (
            lookup
                .answers()
                .iter()
                .map(|record| Answer {
                    name: record.name.to_string(),
                    record_type: record.record_type().to_string(),
                    ttl: record.ttl,
                    fake_ip: is_fake_ip(&record.data.to_string()),
                    value: record.data.to_string(),
                })
                .collect(),
            None,
        ),
        Err(err) => (vec![], Some(err)),
    };
    sort_answers(&mut answers);
    ServerResult {
        server,
        address: address.map(|ip| ip.to_string()),
        ms,
        answers,
        error,
    }
}

/// MX / SRV 按优先级（记录值开头的数字）排序，其余保持服务器返回的顺序
pub fn sort_answers(answers: &mut [Answer]) {
    let priority = |a: &Answer| {
        matches!(a.record_type.as_str(), "MX" | "SRV")
            .then(|| a.value.split_whitespace().next()?.parse::<u32>().ok())
            .flatten()
    };
    answers.sort_by_key(|a| priority(a).unwrap_or(0));
}

pub fn lookup(args: Args) -> PluginResult<Report> {
    let kind = record_type(&args.record_type)?;
    let name = query_name(&args.name, kind)?;
    let mut servers: Vec<String> = Vec::new();
    for server in args
        .servers
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        if !servers.iter().any(|s| s == server) {
            servers.push(server.to_owned());
        }
    }
    if servers.is_empty() {
        return Err(PluginError::new("dns.no_servers"));
    }
    servers.truncate(MAX_SERVERS);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| PluginError::new("dns.failed").with("detail", e.to_string()))?;
    let results = runtime.block_on(async {
        // 各服务器并发查询
        let handles: Vec<_> = servers
            .into_iter()
            .map(|server| tokio::spawn(query(server, name.clone(), kind)))
            .collect();
        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            if let Ok(result) = handle.await {
                results.push(result);
            }
        }
        results
    });

    Ok(Report {
        query: name.to_string(),
        record_type: kind.to_string(),
        results,
    })
}

#[cfg(test)]
#[path = "dns_test.rs"]
mod tests;
