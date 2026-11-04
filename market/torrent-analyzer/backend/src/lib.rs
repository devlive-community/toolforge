//! 种子文件分析器后端：bencode 解码 .torrent 元数据（文件树、infohash、
//! tracker、分块）与 magnet URI 解析。只读用户选择文件，零网络请求。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};
use tf_plugin_guest::net;

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct TorrentAnalyzer {
    manifest: Manifest,
}

impl Default for TorrentAnalyzer {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for TorrentAnalyzer {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "analyze" => to_value(analyze(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(TorrentAnalyzer);

const MAX_FILE: u64 = 4 * 1024 * 1024;

/// bencode 值
#[derive(Debug, Clone)]
enum Bencode {
    Int(i64),
    Bytes(Vec<u8>),
    List(Vec<Bencode>),
    Dict(Vec<(Vec<u8>, Bencode)>),
}

fn bencode_parse(data: &[u8]) -> PluginResult<(Bencode, usize)> {
    if data.is_empty() {
        return Err(PluginError::new("torrent.empty_file"));
    }
    match data[0] {
        b'i' => {
            let end = data
                .iter()
                .position(|b| *b == b'e')
                .ok_or_else(|| PluginError::new("torrent.bad_bencode"))?;
            let text = std::str::from_utf8(&data[1..end])
                .map_err(|_| PluginError::new("torrent.bad_bencode"))?;
            let value: i64 = text
                .parse()
                .map_err(|_| PluginError::new("torrent.bad_bencode"))?;
            Ok((Bencode::Int(value), end + 1))
        }
        b'0'..=b'9' => {
            let colon = data
                .iter()
                .position(|b| *b == b':')
                .ok_or_else(|| PluginError::new("torrent.bad_bencode"))?;
            let len_text = std::str::from_utf8(&data[..colon])
                .map_err(|_| PluginError::new("torrent.bad_bencode"))?;
            let len: usize = len_text
                .parse()
                .map_err(|_| PluginError::new("torrent.bad_bencode"))?;
            let start = colon + 1;
            let end = start + len;
            if end > data.len() {
                return Err(PluginError::new("torrent.bad_bencode"));
            }
            Ok((Bencode::Bytes(data[start..end].to_vec()), end))
        }
        b'l' => {
            let mut list = Vec::new();
            let mut offset = 1;
            loop {
                if offset >= data.len() {
                    return Err(PluginError::new("torrent.bad_bencode"));
                }
                if data[offset] == b'e' {
                    return Ok((Bencode::List(list), offset + 1));
                }
                let (value, consumed) = bencode_parse(&data[offset..])?;
                list.push(value);
                offset += consumed;
            }
        }
        b'd' => {
            let mut dict = Vec::new();
            let mut offset = 1;
            loop {
                if offset >= data.len() {
                    return Err(PluginError::new("torrent.bad_bencode"));
                }
                if data[offset] == b'e' {
                    return Ok((Bencode::Dict(dict), offset + 1));
                }
                let (key, consumed) = bencode_parse(&data[offset..])?;
                offset += consumed;
                let (value, consumed) = bencode_parse(&data[offset..])?;
                offset += consumed;
                dict.push((
                    match key {
                        Bencode::Bytes(b) => b,
                        _ => return Err(PluginError::new("torrent.bad_bencode")),
                    },
                    value,
                ));
            }
        }
        _ => Err(PluginError::new("torrent.bad_bencode")),
    }
}

fn dict_get<'a>(dict: &'a Bencode, key: &str) -> Option<&'a Bencode> {
    match dict {
        Bencode::Dict(items) => items
            .iter()
            .find(|(k, _)| **k == *key.as_bytes())
            .map(|(_, v)| v),
        _ => None,
    }
}

fn bytes_text(value: &Bencode) -> Option<String> {
    match value {
        Bencode::Bytes(bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
        _ => None,
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    /// .torrent 文件路径或 magnet URI（magnet: 开头）
    pub source: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentFile {
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    /// torrent / magnet
    pub kind: String,
    pub display_name: String,
    pub info_hash: String,
    pub piece_length: u64,
    pub piece_count: u64,
    pub total_size: u64,
    pub files: Vec<TorrentFile>,
    pub trackers: Vec<String>,
    pub created_by: Option<String>,
    pub comment: Option<String>,
}

pub fn analyze(args: Args) -> PluginResult<Output> {
    let source = args.source.trim();
    if source.is_empty() {
        return Err(PluginError::new("torrent.empty_source"));
    }
    if let Some(magnet) = source
        .strip_prefix("magnet:")
        .map(|m| m.trim_start_matches('?'))
    {
        return analyze_magnet(magnet);
    }
    let size = net::file_size(source)
        .map_err(|e| PluginError::new("torrent.read_failed").with("detail", e.code))?;
    if size > MAX_FILE {
        return Err(PluginError::new("torrent.too_large").with("max", MAX_FILE));
    }
    let data = net::read_to_end(source, MAX_FILE)
        .map_err(|e| PluginError::new("torrent.read_failed").with("detail", e.code))?;
    let (root, consumed) = bencode_parse(&data)?;
    if consumed != data.len() {
        return Err(PluginError::new("torrent.trailing_data"));
    }
    let Bencode::Dict(_) = &root else {
        return Err(PluginError::new("torrent.bad_bencode"));
    };
    // infohash：对 info 字典的原始字节做 SHA-1（按其在文件中的位置）
    let info_bytes = extract_raw_info(&data).ok_or_else(|| PluginError::new("torrent.no_info"))?;
    let info_hash = hex(&sha1(&info_bytes));
    let display_name = dict_get(&root, "name")
        .and_then(bytes_text)
        .or_else(|| {
            dict_get(
                dict_get(&root, "info").unwrap_or(&Bencode::Bytes(vec![])),
                "name",
            )
            .and_then(bytes_text)
        })
        .unwrap_or_default();
    let info = dict_get(&root, "info").ok_or_else(|| PluginError::new("torrent.no_info"))?;
    let piece_length = match dict_get(info, "piece length") {
        Some(Bencode::Int(value)) => *value as u64,
        _ => return Err(PluginError::new("torrent.no_pieces")),
    };
    let pieces = match dict_get(info, "pieces") {
        Some(Bencode::Bytes(bytes)) => bytes.len() as u64 / 20,
        _ => return Err(PluginError::new("torrent.no_pieces")),
    };
    let mut files = Vec::new();
    let mut total_size = 0u64;
    match dict_get(info, "files") {
        Some(Bencode::List(entries)) => {
            // 多文件种子：path 列表 + length
            for entry in entries {
                let length = match dict_get(entry, "length") {
                    Some(Bencode::Int(value)) => *value as u64,
                    _ => continue,
                };
                let path = match dict_get(entry, "path") {
                    Some(Bencode::List(parts)) => parts
                        .iter()
                        .filter_map(bytes_text)
                        .collect::<Vec<_>>()
                        .join("/"),
                    _ => String::new(),
                };
                total_size += length;
                files.push(TorrentFile { path, size: length });
            }
        }
        _ => {
            // 单文件种子：name + length
            let length = match dict_get(info, "length") {
                Some(Bencode::Int(value)) => *value as u64,
                _ => 0,
            };
            total_size = length;
            files.push(TorrentFile {
                path: display_name.clone(),
                size: length,
            });
        }
    }
    let mut trackers = Vec::new();
    if let Some(Bencode::List(list)) = dict_get(&root, "announce-list")
        && !list.is_empty()
    {
        for entry in list {
            if let Some(url) = bytes_text(entry) {
                trackers.push(url);
            }
        }
    }
    if trackers.is_empty()
        && let Some(url) = dict_get(&root, "announce").and_then(bytes_text)
    {
        trackers.push(url);
    }
    Ok(Output {
        kind: "torrent".into(),
        display_name,
        info_hash,
        piece_length,
        piece_count: pieces,
        total_size,
        files,
        trackers,
        created_by: dict_get(&root, "created by").and_then(bytes_text),
        comment: dict_get(&root, "comment").and_then(bytes_text),
    })
}

/// SHA-1（无依赖实现，RFC 3174 向量在测试中锁定）
fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let ml = (data.len() as u64) * 8;
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&ml.to_be_bytes());
    for chunk in message.chunks(64) {
        let mut w = [0u32; 80];
        for (index, word) in chunk.chunks(4).enumerate() {
            w[index] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for index in 16..80 {
            w[index] = (w[index - 3] ^ w[index - 8] ^ w[index - 14] ^ w[index - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (index, value) in w.iter().enumerate() {
            let (f, k) = match index {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*value);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut out = [0u8; 20];
    for (index, word) in h.iter().enumerate() {
        out[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// 提取 info 字典的原始字节（从 "4:info" 之后到字典结束）
fn extract_raw_info(data: &[u8]) -> Option<Vec<u8>> {
    let marker = b"4:info";
    let mut start = None;
    for index in 0..data.len().saturating_sub(marker.len()) {
        if &data[index..index + marker.len()] == marker {
            start = Some(index + marker.len());
            break;
        }
    }
    let start = start?;
    let (_, consumed) = bencode_parse(&data[start..]).ok()?;
    Some(data[start..start + consumed].to_vec())
}

fn analyze_magnet(magnet: &str) -> PluginResult<Output> {
    let mut display_name = String::new();
    let mut info_hash = String::new();
    let mut trackers = Vec::new();
    for param in magnet.split('&').filter(|p| !p.is_empty()) {
        let Some((key, value)) = param.split_once('=') else {
            continue;
        };
        match key {
            "dn" => display_name = urldecode(value),
            "xt" if value.starts_with("urn:btih:") => {
                info_hash = value.trim_start_matches("urn:btih:").to_owned()
            }
            "tr" => trackers.push(urldecode(value)),
            _ => {}
        }
    }
    if info_hash.is_empty() {
        return Err(PluginError::new("torrent.magnet_no_hash"));
    }
    Ok(Output {
        kind: "magnet".into(),
        display_name,
        info_hash,
        piece_length: 0,
        piece_count: 0,
        total_size: 0,
        files: Vec::new(),
        trackers,
        created_by: None,
        comment: None,
    })
}

fn urldecode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() + 1 && index + 2 < bytes.len() + 1 => {
                if index + 2 < bytes.len() {
                    let hex_text = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or("");
                    if let Ok(byte) = u8::from_str_radix(hex_text, 16) {
                        out.push(byte);
                        index += 3;
                        continue;
                    }
                }
                out.push(bytes[index]);
                index += 1;
            }
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
