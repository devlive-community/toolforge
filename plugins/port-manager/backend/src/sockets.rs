//! 本机套接字：监听中的 TCP 端口、绑定的 UDP 端口，以及可选的已建立连接。

use std::net::IpAddr;

use netstat2::{AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState, get_sockets_info};
use serde::Serialize;
use tf_plugin_api::{PluginError, PluginResult};

#[derive(Debug, Clone, Copy, Serialize, serde::Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Socket {
    pub protocol: Protocol,
    pub local_addr: String,
    pub port: u16,
    pub remote_addr: Option<String>,
    pub remote_port: Option<u16>,
    /// listen / established / time_wait…；UDP 为 bound
    pub state: String,
    pub pids: Vec<u32>,
    /// 只监听本机回环地址，外部无法访问
    pub loopback: bool,
}

fn state_name(state: TcpState) -> String {
    match state {
        TcpState::Listen => "listen".into(),
        TcpState::Established => "established".into(),
        TcpState::SynSent => "syn_sent".into(),
        TcpState::SynReceived => "syn_received".into(),
        TcpState::FinWait1 => "fin_wait1".into(),
        TcpState::FinWait2 => "fin_wait2".into(),
        TcpState::CloseWait => "close_wait".into(),
        TcpState::Closing => "closing".into(),
        TcpState::LastAck => "last_ack".into(),
        TcpState::TimeWait => "time_wait".into(),
        TcpState::Closed => "closed".into(),
        TcpState::DeleteTcb => "delete_tcb".into(),
        TcpState::Unknown => "unknown".into(),
    }
}

fn address(ip: IpAddr) -> String {
    match ip {
        IpAddr::V6(v6) => v6
            .to_ipv4_mapped()
            .map_or_else(|| v6.to_string(), |v4| v4.to_string()),
        IpAddr::V4(v4) => v4.to_string(),
    }
}

/// 读取系统套接字；connections 为 false 时只保留监听中的端口
pub fn list(connections: bool) -> PluginResult<Vec<Socket>> {
    let raw = get_sockets_info(
        AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6,
        ProtocolFlags::TCP | ProtocolFlags::UDP,
    )
    .map_err(|e| PluginError::new("port.list_failed").with("detail", e.to_string()))?;
    let mut sockets: Vec<Socket> = raw
        .into_iter()
        .filter_map(|info| {
            let pids = info.associated_pids.clone();
            match info.protocol_socket_info {
                ProtocolSocketInfo::Tcp(tcp) => {
                    let listening = tcp.state == TcpState::Listen;
                    if !listening && !connections {
                        return None;
                    }
                    Some(Socket {
                        protocol: Protocol::Tcp,
                        loopback: tcp.local_addr.is_loopback(),
                        local_addr: address(tcp.local_addr),
                        port: tcp.local_port,
                        remote_addr: (!listening).then(|| address(tcp.remote_addr)),
                        remote_port: (!listening).then_some(tcp.remote_port),
                        state: state_name(tcp.state),
                        pids,
                    })
                }
                ProtocolSocketInfo::Udp(udp) => Some(Socket {
                    protocol: Protocol::Udp,
                    loopback: udp.local_addr.is_loopback(),
                    local_addr: address(udp.local_addr),
                    port: udp.local_port,
                    remote_addr: None,
                    remote_port: None,
                    state: "bound".into(),
                    pids,
                }),
            }
        })
        .collect();
    sort(&mut sockets);
    Ok(sockets)
}

/// 监听端口在前，按端口、协议、地址排序，并去掉完全相同的重复项
pub fn sort(sockets: &mut Vec<Socket>) {
    sockets.sort_by(|a, b| {
        let rank = |s: &Socket| match s.state.as_str() {
            "listen" => 0,
            "bound" => 1,
            _ => 2,
        };
        rank(a)
            .cmp(&rank(b))
            .then(a.port.cmp(&b.port))
            .then(a.protocol.cmp(&b.protocol))
            .then(a.local_addr.cmp(&b.local_addr))
            .then(a.remote_port.cmp(&b.remote_port))
    });
    sockets.dedup();
}

#[cfg(test)]
#[path = "sockets_test.rs"]
mod tests;
