use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");
const PAGE_SIZE: u128 = 64;
const TWO_TO_128: &str = "340282366920938463463374607431768211456";

pub struct CidrCalculator {
    manifest: Manifest,
}
impl Default for CidrCalculator {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}
#[derive(Deserialize)]
struct CalculateArgs {
    cidr: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SplitArgs {
    cidr: String,
    prefix: u8,
    #[serde(default)]
    page: u64,
}
#[derive(Clone, Copy)]
struct Cidr {
    ip: u128,
    prefix: u8,
    bits: u8,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Network {
    cidr: String,
    version: u8,
    network: String,
    broadcast: String,
    mask: String,
    wildcard: String,
    first_host: String,
    last_host: String,
    addresses: String,
    usable_hosts: String,
    prefix: u8,
}
fn parse_cidr(input: &str) -> PluginResult<Cidr> {
    let (ip, prefix) = input
        .trim()
        .split_once('/')
        .ok_or_else(|| PluginError::new("cidr.invalid"))?;
    let ip: IpAddr = ip.parse().map_err(|_| PluginError::new("cidr.invalid"))?;
    if prefix.is_empty() || !prefix.bytes().all(|b| b.is_ascii_digit()) {
        return Err(PluginError::new("cidr.invalid"));
    }
    let prefix: u8 = prefix
        .parse()
        .map_err(|_| PluginError::new("cidr.invalid"))?;
    let (ip, bits) = match ip {
        IpAddr::V4(ip) => (u128::from(u32::from(ip)), 32),
        IpAddr::V6(ip) => (u128::from(ip), 128),
    };
    if prefix > bits {
        return Err(PluginError::new("cidr.invalid"));
    }
    Ok(Cidr { ip, prefix, bits })
}
fn address(value: u128, bits: u8) -> String {
    if bits == 32 {
        Ipv4Addr::from(value as u32).to_string()
    } else {
        Ipv6Addr::from(value).to_string()
    }
}
fn mask(bits: u8, prefix: u8) -> u128 {
    let all = if bits == 32 {
        u32::MAX as u128
    } else {
        u128::MAX
    };
    if prefix == 0 {
        0
    } else {
        all << (bits - prefix) & all
    }
}
fn count(power: u8) -> String {
    if power == 128 {
        TWO_TO_128.to_owned()
    } else {
        (1u128 << power).to_string()
    }
}
fn network(cidr: Cidr) -> Network {
    let Cidr { ip, prefix, bits } = cidr;
    let all = if bits == 32 {
        u32::MAX as u128
    } else {
        u128::MAX
    };
    let mask = mask(bits, prefix);
    let base = ip & mask;
    let end = base | (all ^ mask);
    let addresses = count(bits - prefix);
    // IPv4 /31 point-to-point links and /32 hosts use all addresses. IPv6 has no broadcast address.
    let (first, last, usable) = if bits == 32 && prefix < 31 {
        (
            base + 1,
            end - 1,
            ((1u128 << (32 - prefix)) - 2).to_string(),
        )
    } else {
        (base, end, addresses.clone())
    };
    Network {
        cidr: format!("{}/{prefix}", address(base, bits)),
        version: if bits == 32 { 4 } else { 6 },
        network: address(base, bits),
        broadcast: address(end, bits),
        mask: address(mask, bits),
        wildcard: address(all ^ mask, bits),
        first_host: address(first, bits),
        last_host: address(last, bits),
        addresses,
        usable_hosts: usable,
        prefix,
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SplitReport {
    total: String,
    page: u64,
    has_next: bool,
    subnets: Vec<Network>,
}
fn split(args: SplitArgs) -> PluginResult<SplitReport> {
    let parent = parse_cidr(&args.cidr)?;
    if args.prefix < parent.prefix || args.prefix > parent.bits {
        return Err(PluginError::new("cidr.invalid_prefix"));
    }
    let difference = args.prefix - parent.prefix;
    let total = count(difference);
    let total_number = if difference == 128 {
        None
    } else {
        Some(1u128 << difference)
    };
    let start = u128::from(args.page) * PAGE_SIZE;
    if total_number.is_some_and(|total| start >= total) {
        return Err(PluginError::new("cidr.invalid_page"));
    }
    let length = total_number.map_or(PAGE_SIZE, |total| (total - start).min(PAGE_SIZE));
    let base = parent.ip & mask(parent.bits, parent.prefix);
    let shift = parent.bits - args.prefix;
    let subnets = (0..length)
        .map(|offset| {
            let index = start + offset;
            let delta = if shift == 128 { 0 } else { index << shift };
            network(Cidr {
                ip: base + delta,
                prefix: args.prefix,
                bits: parent.bits,
            })
        })
        .collect();
    let has_next = args.page < u64::MAX && total_number.is_none_or(|total| start + length < total);
    Ok(SplitReport {
        total,
        page: args.page,
        has_next,
        subnets,
    })
}
impl ToolPlugin for CidrCalculator {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "calculate" => {
                let args: CalculateArgs = parse_args(args)?;
                to_value(network(parse_cidr(&args.cidr)?))
            }
            "split" => to_value(split(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calculates_ipv4_boundaries() {
        let n = network(parse_cidr("192.168.1.42/24").unwrap());
        assert_eq!(
            (
                n.cidr.as_str(),
                n.broadcast.as_str(),
                n.first_host.as_str(),
                n.last_host.as_str(),
                n.usable_hosts.as_str()
            ),
            (
                "192.168.1.0/24",
                "192.168.1.255",
                "192.168.1.1",
                "192.168.1.254",
                "254"
            )
        );
        let n = network(parse_cidr("0.0.0.0/0").unwrap());
        assert_eq!(
            (
                n.addresses.as_str(),
                n.usable_hosts.as_str(),
                n.broadcast.as_str()
            ),
            ("4294967296", "4294967294", "255.255.255.255")
        );
        assert_eq!(
            network(parse_cidr("255.255.255.255/31").unwrap()).usable_hosts,
            "2"
        );
        assert_eq!(
            network(parse_cidr("255.255.255.255/32").unwrap()).first_host,
            "255.255.255.255"
        );
    }
    #[test]
    fn calculates_ipv6_boundaries() {
        let n = network(parse_cidr("2001:db8:1234:5678::abcd/64").unwrap());
        assert_eq!(
            (
                n.cidr.as_str(),
                n.first_host.as_str(),
                n.last_host.as_str(),
                n.addresses.as_str()
            ),
            (
                "2001:db8:1234:5678::/64",
                "2001:db8:1234:5678::",
                "2001:db8:1234:5678:ffff:ffff:ffff:ffff",
                "18446744073709551616"
            )
        );
        let n = network(parse_cidr("::1/0").unwrap());
        assert_eq!(n.addresses, TWO_TO_128);
        assert_eq!(n.broadcast, "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff");
        assert_eq!(network(parse_cidr("::1/128").unwrap()).usable_hosts, "1");
    }
    #[test]
    fn validates_and_pages_splits() {
        for input in [
            "192.168.1.1",
            "192.168.1.256/24",
            "1.2.3.4/33",
            "::1/129",
            "1.2.3.4/-1",
        ] {
            assert!(parse_cidr(input).is_err());
        }
        let s = split(SplitArgs {
            cidr: "10.1.2.3/16".into(),
            prefix: 24,
            page: 1,
        })
        .unwrap();
        assert_eq!(s.total, "256");
        assert_eq!(s.subnets[0].cidr, "10.1.64.0/24");
        assert_eq!(s.subnets[63].cidr, "10.1.127.0/24");
        let s = split(SplitArgs {
            cidr: "2001:db8::/120".into(),
            prefix: 128,
            page: 3,
        })
        .unwrap();
        assert_eq!(s.subnets[0].cidr, "2001:db8::c0/128");
        assert!(!s.has_next);
        let s = split(SplitArgs {
            cidr: "::/0".into(),
            prefix: 128,
            page: 0,
        })
        .unwrap();
        assert_eq!(s.total, TWO_TO_128);
        assert!(s.has_next);
        assert_eq!(
            split(SplitArgs {
                cidr: "::/0".into(),
                prefix: 0,
                page: 0
            })
            .unwrap()
            .subnets[0]
                .cidr,
            "::/0"
        );
        assert!(
            split(SplitArgs {
                cidr: "10.0.0.0/24".into(),
                prefix: 23,
                page: 0
            })
            .is_err()
        );
        assert!(
            split(SplitArgs {
                cidr: "::/128".into(),
                prefix: 128,
                page: 1
            })
            .is_err()
        );
    }
}
