//! PCAP 查看器后端：经典 PCAP 容器（大小端、微秒/纳秒）解析，
//! Ethernet → IPv4/IPv6 → TCP/UDP/ICMP 字段解读与双向会话汇总。
//! 只读用户选择文件（fs:user-selected），首版不做 PCAPNG 与应用层解析。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};
use tf_plugin_guest::net;

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct PcapViewer {
    manifest: Manifest,
}

impl Default for PcapViewer {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for PcapViewer {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "parse" => to_value(parse(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(PcapViewer);

const MAX_FILE: u64 = 32 * 1024 * 1024;
const MAX_PACKETS: usize = 5_000;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub path: String,
    /// 最多解析的报文数（1–5000，默认 2000）
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Packet {
    pub index: usize,
    /// 秒（Unix epoch）
    pub ts_sec: u64,
    /// 微秒部分
    pub ts_usec: u64,
    pub src: String,
    pub dst: String,
    /// TCP / UDP / ICMP / IPv4 / IPv6 / Ethernet / Truncated
    pub protocol: String,
    pub src_port: Option<u16>,
    pub dst_port: Option<u16>,
    /// TCP flags 文本（如 SYN|ACK）
    pub tcp_flags: Option<String>,
    pub captured_len: usize,
    pub original_len: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub key: String,
    pub packets: usize,
    pub bytes: u64,
    pub first_sec: u64,
    pub last_sec: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub link_type: String,
    pub nanosecond: bool,
    pub big_endian: bool,
    pub packets: Vec<Packet>,
    pub conversations: Vec<Conversation>,
    pub total: usize,
    pub truncated_count: usize,
}

fn ipv4(bytes: &[u8]) -> String {
    format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3])
}

fn tcp_flags_text(flags: u8) -> String {
    let mut out = Vec::new();
    if flags & 0x01 != 0 {
        out.push("FIN");
    }
    if flags & 0x02 != 0 {
        out.push("SYN");
    }
    if flags & 0x04 != 0 {
        out.push("RST");
    }
    if flags & 0x08 != 0 {
        out.push("PSH");
    }
    if flags & 0x10 != 0 {
        out.push("ACK");
    }
    if flags & 0x20 != 0 {
        out.push("URG");
    }
    out.join("|")
}

pub fn parse(args: Args) -> PluginResult<Output> {
    let file_size = net::file_size(&args.path).map_err(|e| {
        if e.code == "fs.not_found" {
            e
        } else {
            PluginError::new("pcap.read_failed").with("detail", e.code)
        }
    })?;
    if file_size > MAX_FILE {
        return Err(PluginError::new("pcap.too_large").with("max", MAX_FILE));
    }
    let data = net::read_to_end(&args.path, MAX_FILE)
        .map_err(|e| PluginError::new("pcap.read_failed").with("detail", e.code))?;
    if data.len() < 24 {
        return Err(PluginError::new("pcap.too_small"));
    }
    let magic = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    let (big_endian, nanosecond) = match magic {
        0xa1b2c3d4 => (true, false),
        0xd4c3b1a2 => (false, false),
        0xa1b23c4d => (true, true),
        0x4d3cb2a1 => (false, true),
        _ => return Err(PluginError::new("pcap.bad_magic").with("magic", format!("{magic:#x}"))),
    };
    let read_u32 = |b: &[u8]| -> u32 {
        if big_endian {
            u32::from_be_bytes([b[0], b[1], b[2], b[3]])
        } else {
            u32::from_le_bytes([b[0], b[1], b[2], b[3]])
        }
    };
    let link_type = read_u32(&data[20..24]);
    let link = match link_type {
        1 => "ethernet".to_owned(),
        101 => "raw-ip".to_owned(),
        other => return Err(PluginError::new("pcap.bad_link_type").with("linkType", other)),
    };
    let limit = args.limit.unwrap_or(2000).clamp(1, MAX_PACKETS);
    let mut packets: Vec<Packet> = Vec::new();
    let mut conversations: std::collections::HashMap<String, Conversation> =
        std::collections::HashMap::new();
    let mut truncated_count = 0usize;
    let mut offset = 24usize;
    let mut index = 0usize;
    while offset + 16 <= data.len() && index < limit {
        let ts_sec = read_u32(&data[offset..offset + 4]) as u64;
        let ts_usec = read_u32(&data[offset + 4..offset + 8]) as u64;
        let captured = read_u32(&data[offset + 8..offset + 12]) as usize;
        let original = read_u32(&data[offset + 12..offset + 16]) as usize;
        if captured > 262_144 || original > 262_144 {
            return Err(PluginError::new("pcap.corrupt_length").with("offset", offset as u64));
        }
        offset += 16;
        if offset + captured > data.len() {
            truncated_count += 1;
            break;
        }
        let body = &data[offset..offset + captured];
        offset += captured;
        index += 1;
        let mut packet = Packet {
            index,
            ts_sec,
            ts_usec,
            src: String::new(),
            dst: String::new(),
            protocol: link.clone(),
            src_port: None,
            dst_port: None,
            tcp_flags: None,
            captured_len: captured,
            original_len: original,
        };
        if captured != original {
            truncated_count += 1;
        }
        // Ethernet（14 字节头，type 在 12..14）
        if link == "ethernet" && captured >= 14 {
            let ethertype = u16::from_be_bytes([body[12], body[13]]);
            let payload = &body[14..];
            match ethertype {
                0x0800 if payload.len() >= 20 => {
                    let ihl = (payload[0] & 0x0f) as usize * 4;
                    let proto = payload[9];
                    if payload.len() >= ihl {
                        packet.src = ipv4(&payload[12..16]);
                        packet.dst = ipv4(&payload[16..20]);
                        let l4 = &payload[ihl..];
                        match proto {
                            6 if l4.len() >= 20 => {
                                packet.protocol = "TCP".into();
                                packet.src_port = Some(u16::from_be_bytes([l4[0], l4[1]]));
                                packet.dst_port = Some(u16::from_be_bytes([l4[2], l4[3]]));
                                packet.tcp_flags = Some(tcp_flags_text(l4[13]));
                            }
                            17 if l4.len() >= 8 => {
                                packet.protocol = "UDP".into();
                                packet.src_port = Some(u16::from_be_bytes([l4[0], l4[1]]));
                                packet.dst_port = Some(u16::from_be_bytes([l4[2], l4[3]]));
                            }
                            1 => packet.protocol = "ICMP".into(),
                            other => packet.protocol = format!("ip-{other}"),
                        }
                    }
                }
                0x86dd if payload.len() >= 40 => {
                    packet.protocol = "IPv6".into();
                    packet.src = format!("{}:…", hex(&payload[8..12]));
                    packet.dst = format!("{}:…", hex(&payload[24..28]));
                }
                _ => {}
            }
        }
        // 会话聚合
        let (a, b) = (packet.src.clone(), packet.dst.clone());
        let key = if a <= b {
            format!(
                "{a}↔{b} {}:{}",
                packet.protocol,
                packet.src_port.unwrap_or(0)
            )
        } else {
            format!(
                "{b}↔{a} {}:{}",
                packet.protocol,
                packet.src_port.unwrap_or(0)
            )
        };
        let entry = conversations.entry(key).or_insert(Conversation {
            key: String::new(),
            packets: 0,
            bytes: 0,
            first_sec: packet.ts_sec,
            last_sec: packet.ts_sec,
        });
        entry.packets += 1;
        entry.bytes += captured as u64;
        entry.last_sec = entry.last_sec.max(packet.ts_sec);
        entry.first_sec = entry.first_sec.min(packet.ts_sec);
        packets.push(packet);
    }
    let mut conversations: Vec<Conversation> = conversations.into_values().collect();
    for conversation in &mut conversations {
        conversation.key = conversation.key.clone();
    }
    conversations.sort_by_key(|c| std::cmp::Reverse(c.packets));
    conversations.truncate(50);
    for conversation in &mut conversations {
        conversation.key = conversation.key.clone();
    }
    let total = packets.len();
    Ok(Output {
        link_type: link,
        nanosecond,
        big_endian,
        packets,
        conversations,
        total,
        truncated_count,
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join("")
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
