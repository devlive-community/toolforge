//! 访问地址：本机地址、局域网地址（附二维码）以及端口占用检查。

use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};

use qrcode::QrCode;
use qrcode::render::svg;
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub port: u16,
    #[serde(default)]
    pub lan: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Address {
    pub url: String,
    /// local / lan
    pub kind: &'static str,
    /// 网卡名称（局域网地址）
    pub interface: Option<String>,
    /// 手机扫码访问用的二维码（局域网地址）
    pub qr: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Addresses {
    pub addresses: Vec<Address>,
    /// 端口当前是否可以监听
    pub available: bool,
}

/// 绑定的地址：默认仅本机，开启局域网访问时监听所有 IPv4 网卡
pub fn bind_addr(port: u16, lan: bool) -> SocketAddr {
    let ip = if lan {
        Ipv4Addr::UNSPECIFIED
    } else {
        Ipv4Addr::LOCALHOST
    };
    SocketAddr::from((ip, port))
}

pub fn bind(port: u16, lan: bool) -> PluginResult<TcpListener> {
    if port == 0 {
        return Err(PluginError::new("server.invalid_port"));
    }
    TcpListener::bind(bind_addr(port, lan)).map_err(|e| {
        let code = match e.kind() {
            std::io::ErrorKind::AddrInUse => "server.port_in_use",
            std::io::ErrorKind::PermissionDenied => "server.port_denied",
            _ => "server.bind_failed",
        };
        PluginError::new(code)
            .with("port", port)
            .with("detail", e.to_string())
    })
}

/// 代理软件的 fake-ip / TUN 网段（198.18.0.0/15）不是真实的局域网地址
fn usable_lan(ip: &Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    !ip.is_loopback()
        && !ip.is_link_local()
        && !ip.is_unspecified()
        && !(a == 198 && (b & 0xfe) == 18)
}

/// 虚拟机、容器与 VPN 的虚拟网卡排在物理网卡之后
fn is_virtual(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "bridge",
        "vmnet",
        "docker",
        "veth",
        "vboxnet",
        "utun",
        "tun",
        "tap",
        "br-",
        "vethernet",
        "virtualbox",
        "vmware",
    ]
    .iter()
    .any(|prefix| name.starts_with(prefix))
}

fn rank(ip: &Ipv4Addr) -> u8 {
    match ip.octets() {
        [192, 168, ..] => 0,
        [10, ..] => 1,
        [172, b, ..] if (16..32).contains(&b) => 2,
        _ => 3,
    }
}

/// 二维码图片（SVG 的 data URI，自带白色底）
pub fn qr_image(text: &str) -> Option<String> {
    let code = QrCode::new(text.as_bytes()).ok()?;
    let svg = code
        .render::<svg::Color>()
        .min_dimensions(180, 180)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();
    Some(format!(
        "data:image/svg+xml;base64,{}",
        data_encoding::BASE64.encode(svg.as_bytes())
    ))
}

pub fn lan_ips() -> Vec<(String, Ipv4Addr)> {
    let mut ips: Vec<(String, Ipv4Addr)> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|iface| match iface.ip() {
            IpAddr::V4(ip) if usable_lan(&ip) => Some((iface.name, ip)),
            _ => None,
        })
        .collect();
    ips.sort_by_key(|(name, ip)| (is_virtual(name), rank(ip), *ip));
    ips.dedup_by_key(|(_, ip)| *ip);
    ips
}

pub fn addresses(args: Args) -> PluginResult<Addresses> {
    if args.port == 0 {
        return Err(PluginError::new("server.invalid_port"));
    }
    let port = args.port;
    let mut addresses = vec![Address {
        url: format!("http://localhost:{port}/"),
        kind: "local",
        interface: None,
        qr: None,
    }];
    if args.lan {
        addresses.extend(lan_ips().into_iter().map(|(name, ip)| {
            let url = format!("http://{ip}:{port}/");
            Address {
                qr: qr_image(&url),
                url,
                kind: "lan",
                interface: Some(name),
            }
        }));
    }
    let available = TcpListener::bind(bind_addr(port, args.lan)).is_ok();
    Ok(Addresses {
        addresses,
        available,
    })
}

/// 从给定端口开始向上寻找一个可用端口
pub fn free_port(start: u16, lan: bool) -> PluginResult<u16> {
    (start.max(1024)..=u16::MAX)
        .take(200)
        .find(|port| TcpListener::bind(bind_addr(*port, lan)).is_ok())
        .ok_or_else(|| PluginError::new("server.no_free_port"))
}

#[cfg(test)]
#[path = "addresses_test.rs"]
mod tests;
