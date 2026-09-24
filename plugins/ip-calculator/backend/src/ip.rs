use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

/// 子网拆分最多列出的数量
const MAX_SUBNETS: usize = 256;
/// 地址段转 CIDR 最多输出的数量
const MAX_CIDRS: usize = 512;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    input: String,
    /// 拆分为更小的前缀（例如 /24 拆成 /26）
    #[serde(default)]
    split: Option<u8>,
    /// 检查该地址是否属于网段
    #[serde(default)]
    contains: Option<String>,
}

#[derive(Deserialize)]
pub struct RangeArgs {
    start: String,
    end: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Subnet {
    pub cidr: String,
    pub first: String,
    pub last: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Membership {
    pub address: String,
    pub inside: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub version: u8,
    pub address: String,
    pub prefix: u8,
    pub cidr: String,
    pub network: String,
    /// 仅 IPv4
    pub broadcast: Option<String>,
    pub netmask: String,
    pub wildcard: String,
    pub first_host: String,
    pub last_host: String,
    /// 地址总数与可用主机数（IPv6 可能极大，用字符串表示）
    pub total: String,
    pub usable: String,
    /// private / loopback / link_local / multicast / documentation / shared / unspecified / broadcast / reserved / unique_local / global
    pub kind: &'static str,
    /// IPv4 的传统分类 A-E
    pub class: Option<char>,
    pub binary: String,
    pub hex: String,
    pub integer: String,
    pub reverse_dns: String,
    /// IPv6 的完整写法；IPv4 为对应的 IPv4 映射 IPv6 地址
    pub expanded: String,
    pub subnets: Vec<Subnet>,
    /// 拆分后的子网总数（可能多于列出的数量）
    pub subnet_count: Option<String>,
    pub membership: Option<Membership>,
}

#[derive(Debug, Serialize)]
pub struct RangeReport {
    pub cidrs: Vec<String>,
    pub total: String,
}

fn invalid(input: &str) -> PluginError {
    PluginError::new("ip.invalid").with("input", input)
}

/// 统一用 u128 运算；IPv4 占低 32 位
#[derive(Debug, Clone, Copy, PartialEq)]
struct Net {
    bits: u8,
    addr: u128,
    prefix: u8,
}

impl Net {
    fn mask(&self) -> u128 {
        if self.prefix == 0 {
            0
        } else {
            (u128::MAX << (128 - self.prefix as u32)) >> (128 - self.bits as u32)
        }
    }

    fn full(&self) -> u128 {
        if self.bits == 128 {
            u128::MAX
        } else {
            (1u128 << self.bits) - 1
        }
    }

    fn network(&self) -> u128 {
        self.addr & self.mask()
    }

    fn last(&self) -> u128 {
        self.network() | (!self.mask() & self.full())
    }

    fn format(&self, value: u128) -> String {
        to_ip(self.bits, value).to_string()
    }
}

fn to_ip(bits: u8, value: u128) -> IpAddr {
    if bits == 32 {
        IpAddr::V4(Ipv4Addr::from(value as u32))
    } else {
        IpAddr::V6(Ipv6Addr::from(value))
    }
}

fn from_ip(ip: IpAddr) -> (u8, u128) {
    match ip {
        IpAddr::V4(v4) => (32, u32::from(v4) as u128),
        IpAddr::V6(v6) => (128, u128::from(v6)),
    }
}

/// 子网掩码转前缀长度；掩码必须是连续的 1
fn mask_to_prefix(mask: Ipv4Addr) -> Option<u8> {
    let value = u32::from(mask);
    let prefix = value.leading_ones();
    (value.checked_shl(prefix).unwrap_or(0) == 0).then_some(prefix as u8)
}

/// 解析 `地址`、`地址/前缀`、`地址/掩码` 或 `地址 掩码`
fn parse_net(input: &str) -> PluginResult<Net> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(PluginError::new("ip.empty"));
    }
    let (addr, suffix) = match trimmed.split_once('/') {
        Some((a, s)) => (a.trim(), Some(s.trim())),
        None => match trimmed.split_once(char::is_whitespace) {
            Some((a, s)) => (a.trim(), Some(s.trim())),
            None => (trimmed, None),
        },
    };
    let ip: IpAddr = addr.parse().map_err(|_| invalid(addr))?;
    let (bits, value) = from_ip(ip);
    let prefix = match suffix {
        None => bits,
        Some(s) => match s.parse::<u8>() {
            Ok(p) if p <= bits => p,
            Ok(_) => {
                return Err(PluginError::new("ip.invalid_prefix")
                    .with("prefix", s)
                    .with("max", bits));
            }
            Err(_) => {
                let mask: Ipv4Addr = s.parse().ok().filter(|_| bits == 32).ok_or_else(|| {
                    PluginError::new("ip.invalid_prefix")
                        .with("prefix", s)
                        .with("max", bits)
                })?;
                mask_to_prefix(mask)
                    .ok_or_else(|| PluginError::new("ip.invalid_mask").with("mask", s))?
            }
        },
    };
    Ok(Net {
        bits,
        addr: value,
        prefix,
    })
}

fn kind(ip: IpAddr) -> &'static str {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, c, _] = v4.octets();
            if v4.is_unspecified() {
                "unspecified"
            } else if v4.is_loopback() {
                "loopback"
            } else if v4.is_private() {
                "private"
            } else if v4.is_link_local() {
                "link_local"
            } else if v4.is_multicast() {
                "multicast"
            } else if v4.is_broadcast() {
                "broadcast"
            } else if a == 100 && (64..128).contains(&b) {
                "shared"
            } else if (a, b, c) == (192, 0, 2)
                || (a, b, c) == (198, 51, 100)
                || (a, b, c) == (203, 0, 113)
            {
                "documentation"
            } else if a >= 240 || (a == 198 && (b == 18 || b == 19)) {
                "reserved"
            } else {
                "global"
            }
        }
        IpAddr::V6(v6) => {
            let seg = v6.segments();
            if v6.is_unspecified() {
                "unspecified"
            } else if v6.is_loopback() {
                "loopback"
            } else if v6.is_multicast() {
                "multicast"
            } else if seg[0] & 0xffc0 == 0xfe80 {
                "link_local"
            } else if seg[0] & 0xfe00 == 0xfc00 {
                "unique_local"
            } else if seg[0] == 0x2001 && seg[1] == 0x0db8 {
                "documentation"
            } else if v6.to_ipv4_mapped().is_some() {
                "reserved"
            } else {
                "global"
            }
        }
    }
}

fn class(ip: Ipv4Addr) -> char {
    match ip.octets()[0] {
        0..=127 => 'A',
        128..=191 => 'B',
        192..=223 => 'C',
        224..=239 => 'D',
        _ => 'E',
    }
}

fn binary(bits: u8, value: u128) -> String {
    let raw = format!("{value:0width$b}", width = bits as usize);
    let group = if bits == 32 { 8 } else { 16 };
    raw.as_bytes()
        .chunks(group)
        .map(|c| std::str::from_utf8(c).unwrap_or_default())
        .collect::<Vec<_>>()
        .join(if bits == 32 { "." } else { ":" })
}

fn reverse_dns(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, c, d] = v4.octets();
            format!("{d}.{c}.{b}.{a}.in-addr.arpa")
        }
        IpAddr::V6(v6) => {
            let hex = format!("{:032x}", u128::from(v6));
            let mut nibbles: Vec<String> = hex.chars().rev().map(String::from).collect();
            nibbles.push("ip6.arpa".into());
            nibbles.join(".")
        }
    }
}

fn expanded(ip: IpAddr) -> String {
    let v6 = match ip {
        IpAddr::V4(v4) => v4.to_ipv6_mapped(),
        IpAddr::V6(v6) => v6,
    };
    v6.segments()
        .iter()
        .map(|s| format!("{s:04x}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// 2^n 的十进制字符串（n 最大 128）
fn pow2(n: u32) -> String {
    if n < 128 {
        (1u128 << n).to_string()
    } else {
        "340282366920938463463374607431768211456".into()
    }
}

fn split(net: &Net, new_prefix: u8) -> PluginResult<(Vec<Subnet>, String)> {
    if new_prefix < net.prefix || new_prefix > net.bits {
        return Err(PluginError::new("ip.invalid_split")
            .with("prefix", new_prefix)
            .with("min", net.prefix)
            .with("max", net.bits));
    }
    let count_bits = (new_prefix - net.prefix) as u32;
    let size_bits = (net.bits - new_prefix) as u32;
    let base = net.network();
    let listed = if count_bits >= 16 {
        MAX_SUBNETS
    } else {
        (1usize << count_bits).min(MAX_SUBNETS)
    };
    let subnets = (0..listed as u128)
        .map(|i| {
            let start = base + if size_bits >= 128 { 0 } else { i << size_bits };
            let sub = Net {
                bits: net.bits,
                addr: start,
                prefix: new_prefix,
            };
            Subnet {
                cidr: format!("{}/{new_prefix}", sub.format(sub.network())),
                first: sub.format(sub.network()),
                last: sub.format(sub.last()),
            }
        })
        .collect();
    Ok((subnets, pow2(count_bits)))
}

pub fn calculate(args: Args) -> PluginResult<Report> {
    let net = parse_net(&args.input)?;
    let ip = to_ip(net.bits, net.addr);
    let (network, last) = (net.network(), net.last());
    let host_bits = (net.bits - net.prefix) as u32;
    let v4 = net.bits == 32;

    // IPv4 的 /31、/32 没有网络与广播地址之分（RFC 3021）
    let (first_host, last_host, usable) = if v4 && host_bits >= 2 {
        (
            network + 1,
            last - 1,
            ((1u128 << host_bits) - 2).to_string(),
        )
    } else {
        (network, last, pow2(host_bits))
    };

    let (subnets, subnet_count) = match args.split {
        Some(prefix) => {
            let (subnets, count) = split(&net, prefix)?;
            (subnets, Some(count))
        }
        None => (Vec::new(), None),
    };
    let membership = match args
        .contains
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(other) => {
            let parsed: IpAddr = other
                .parse()
                .map_err(|_| invalid(other).with("field", "contains"))?;
            let (bits, value) = from_ip(parsed);
            Some(Membership {
                address: parsed.to_string(),
                inside: bits == net.bits && value & net.mask() == network,
            })
        }
        None => None,
    };

    Ok(Report {
        version: if v4 { 4 } else { 6 },
        address: ip.to_string(),
        prefix: net.prefix,
        cidr: format!("{}/{}", net.format(network), net.prefix),
        network: net.format(network),
        broadcast: v4.then(|| net.format(last)),
        netmask: net.format(net.mask()),
        wildcard: net.format(!net.mask() & net.full()),
        first_host: net.format(first_host),
        last_host: net.format(last_host),
        total: pow2(host_bits),
        usable,
        kind: kind(ip),
        class: match ip {
            IpAddr::V4(v4) => Some(class(v4)),
            IpAddr::V6(_) => None,
        },
        binary: binary(net.bits, net.addr),
        hex: if v4 {
            format!("0x{:08X}", net.addr)
        } else {
            format!("0x{:032X}", net.addr)
        },
        integer: net.addr.to_string(),
        reverse_dns: reverse_dns(ip),
        expanded: expanded(ip),
        subnets,
        subnet_count,
        membership,
    })
}

/// 把任意地址段转换为最少数量的 CIDR
pub fn range_to_cidrs(args: RangeArgs) -> PluginResult<RangeReport> {
    let parse = |s: &str| -> PluginResult<(u8, u128)> {
        s.trim()
            .parse::<IpAddr>()
            .map(from_ip)
            .map_err(|_| invalid(s.trim()))
    };
    let (bits, start) = parse(&args.start)?;
    let (end_bits, end) = parse(&args.end)?;
    if bits != end_bits {
        return Err(PluginError::new("ip.version_mismatch"));
    }
    if start > end {
        return Err(PluginError::new("ip.range_reversed"));
    }
    let mut cidrs = Vec::new();
    let mut current = start;
    loop {
        // 当前地址能对齐的最大块，且不能超过终点
        let align = if current == 0 {
            bits as u32
        } else {
            current.trailing_zeros().min(bits as u32)
        };
        let remaining = end - current;
        let fit = if remaining == u128::MAX {
            128
        } else {
            127 - (remaining + 1).leading_zeros()
        };
        let size = align.min(fit);
        cidrs.push(format!("{}/{}", to_ip(bits, current), bits as u32 - size));
        if cidrs.len() >= MAX_CIDRS {
            return Err(PluginError::new("ip.too_many_cidrs").with("limit", MAX_CIDRS));
        }
        let block = if size >= 128 {
            u128::MAX
        } else {
            (1u128 << size) - 1
        };
        match current.checked_add(block) {
            Some(last) if last < end => current = last + 1,
            _ => break,
        }
    }
    let total = if end - start == u128::MAX {
        pow2(128)
    } else {
        (end - start + 1).to_string()
    };
    Ok(RangeReport { cidrs, total })
}

#[cfg(test)]
#[path = "ip_test.rs"]
mod tests;
