//! 端口连通性检查：解析「80,443,8000-8010」形式的端口列表，并行尝试 TCP 连接。

use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult, TaskContext, cancelled};

pub const MAX_PORTS: usize = 1024;
const WORKERS: usize = 64;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub host: String,
    pub ports: String,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    1500
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PortResult {
    pub port: u16,
    pub open: bool,
    pub ms: Option<u64>,
    /// refused / timeout / unreachable / other
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub host: String,
    /// 解析出的地址，逗号分隔
    pub address: String,
    pub results: Vec<PortResult>,
    pub open: usize,
    /// 解析结果位于 198.18.0.0/15：通常是代理软件的 fake-ip，连接结果反映的是代理而非真实服务器
    pub fake_ip: bool,
    pub elapsed_ms: u64,
}

fn invalid(value: &str) -> PluginError {
    PluginError::new("port.invalid_ports").with("value", value)
}

/// 解析端口列表：逗号或空白分隔，支持 a-b 区间，去重并排序
pub fn parse_ports(spec: &str) -> PluginResult<Vec<u16>> {
    let mut ports = Vec::new();
    for part in spec
        .split([',', ' ', '\n', '\t'])
        .filter(|p| !p.trim().is_empty())
    {
        let part = part.trim();
        let (start, end) = match part.split_once('-') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (part, part),
        };
        let start: u16 = start.parse().map_err(|_| invalid(part))?;
        let end: u16 = end.parse().map_err(|_| invalid(part))?;
        if start == 0 || end < start {
            return Err(invalid(part));
        }
        if ports.len() + (end - start) as usize + 1 > MAX_PORTS {
            return Err(PluginError::new("port.too_many_ports").with("max", MAX_PORTS));
        }
        ports.extend(start..=end);
    }
    ports.sort_unstable();
    ports.dedup();
    if ports.is_empty() {
        return Err(PluginError::new("port.no_ports"));
    }
    Ok(ports)
}

pub fn is_fake_ip(address: &SocketAddr) -> bool {
    matches!(address.ip(), std::net::IpAddr::V4(v4) if v4.octets()[0] == 198 && (v4.octets()[1] & 0xfe) == 18)
}

/// 解析主机的全部地址（去重，最多 4 个）；localhost 通常同时有 ::1 与 127.0.0.1
fn resolve(host: &str) -> PluginResult<Vec<SocketAddr>> {
    let host = host.trim().trim_start_matches('[').trim_end_matches(']');
    if host.is_empty() {
        return Err(PluginError::new("port.no_host"));
    }
    let mut addresses: Vec<SocketAddr> = Vec::new();
    for address in (host, 0).to_socket_addrs().into_iter().flatten() {
        if !addresses.contains(&address) {
            addresses.push(address);
        }
    }
    addresses.truncate(4);
    if addresses.is_empty() {
        return Err(PluginError::new("port.resolve_failed").with("host", host));
    }
    Ok(addresses)
}

fn reason(err: &std::io::Error) -> &'static str {
    match err.kind() {
        std::io::ErrorKind::ConnectionRefused => "refused",
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => "timeout",
        std::io::ErrorKind::HostUnreachable | std::io::ErrorKind::NetworkUnreachable => {
            "unreachable"
        }
        _ => "other",
    }
}

/// 依次尝试每个地址，任一地址连接成功即视为开放；都失败时返回第一个地址的原因
fn probe(addresses: &[SocketAddr], port: u16, timeout: Duration) -> PortResult {
    let mut first_reason = None;
    for address in addresses {
        let start = Instant::now();
        match TcpStream::connect_timeout(&SocketAddr::new(address.ip(), port), timeout) {
            Ok(_) => {
                return PortResult {
                    port,
                    open: true,
                    ms: Some(start.elapsed().as_millis() as u64),
                    reason: None,
                };
            }
            Err(err) => {
                first_reason.get_or_insert(reason(&err));
            }
        }
    }
    PortResult {
        port,
        open: false,
        ms: None,
        reason: Some(first_reason.unwrap_or("other").into()),
    }
}

pub fn run(args: Args, ctx: &dyn TaskContext) -> PluginResult<Report> {
    let start = Instant::now();
    let ports = parse_ports(&args.ports)?;
    let addresses = resolve(&args.host)?;
    let timeout = Duration::from_millis(args.timeout_ms.clamp(100, 10_000));
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let results = Mutex::new(Vec::with_capacity(ports.len()));
    std::thread::scope(|scope| {
        for _ in 0..WORKERS.min(ports.len()) {
            scope.spawn(|| {
                while let Some(&port) = ports.get(next.fetch_add(1, Ordering::Relaxed)) {
                    if ctx.is_cancelled() {
                        break;
                    }
                    let result = probe(&addresses, port, timeout);
                    results
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push(result);
                    let finished = done.fetch_add(1, Ordering::Relaxed) + 1;
                    ctx.progress(finished as u64, ports.len() as u64);
                }
            });
        }
    });
    if ctx.is_cancelled() {
        return Err(cancelled());
    }
    let mut results = results.into_inner().unwrap_or_else(|e| e.into_inner());
    results.sort_by_key(|r| r.port);
    Ok(Report {
        host: args.host.trim().to_owned(),
        address: addresses
            .iter()
            .map(|a| a.ip().to_string())
            .collect::<Vec<_>>()
            .join(", "),
        open: results.iter().filter(|r| r.open).count(),
        fake_ip: addresses.iter().any(is_fake_ip),
        results,
        elapsed_ms: start.elapsed().as_millis() as u64,
    })
}

#[cfg(test)]
#[path = "check_test.rs"]
mod tests;
