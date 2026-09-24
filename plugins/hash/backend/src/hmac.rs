use std::time::Instant;

use digest::Digest;
use digest::block_api::BlockSizeUser;
use hmac::{KeyInit, Mac, SimpleHmac};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::algo::Algorithm;

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum KeyEncoding {
    #[default]
    Text,
    Hex,
    Base64,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OutputEncoding {
    #[default]
    Hex,
    Base64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    input: String,
    key: String,
    #[serde(default)]
    key_encoding: KeyEncoding,
    algorithms: Vec<Algorithm>,
    #[serde(default)]
    output: OutputEncoding,
    #[serde(default)]
    uppercase: bool,
    /// 期望的 MAC（任意编码），用于校验
    #[serde(default)]
    expected: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub algorithm: Algorithm,
    pub mac: String,
    /// 与期望值一致（未提供期望值时为空）
    pub matches: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub results: Vec<Item>,
    pub key_bytes: usize,
    pub elapsed_ms: f64,
}

fn compute<D: Digest + BlockSizeUser + Clone>(key: &[u8], data: &[u8]) -> Vec<u8> {
    // HMAC 接受任意长度的密钥
    let mut mac =
        <SimpleHmac<D> as KeyInit>::new_from_slice(key).expect("hmac accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

pub fn mac(algorithm: Algorithm, key: &[u8], data: &[u8]) -> Option<Vec<u8>> {
    Some(match algorithm {
        Algorithm::Md5 => compute::<md5::Md5>(key, data),
        Algorithm::Sha1 => compute::<sha1::Sha1>(key, data),
        Algorithm::Sha224 => compute::<sha2::Sha224>(key, data),
        Algorithm::Sha256 => compute::<sha2::Sha256>(key, data),
        Algorithm::Sha384 => compute::<sha2::Sha384>(key, data),
        Algorithm::Sha512 => compute::<sha2::Sha512>(key, data),
        Algorithm::Sha3_256 => compute::<sha3::Sha3_256>(key, data),
        Algorithm::Sha3_512 => compute::<sha3::Sha3_512>(key, data),
        Algorithm::Sm3 => compute::<sm3::Sm3>(key, data),
        // CRC32 不是密码学哈希，没有 HMAC
        Algorithm::Crc32 => return None,
    })
}

fn decode_key(key: &str, encoding: KeyEncoding) -> PluginResult<Vec<u8>> {
    let invalid = || {
        PluginError::new("hmac.invalid_key")
            .with("encoding", format!("{encoding:?}").to_lowercase())
    };
    match encoding {
        KeyEncoding::Text => Ok(key.as_bytes().to_vec()),
        KeyEncoding::Hex => {
            let clean: String = key.chars().filter(|c| !c.is_whitespace()).collect();
            hex::decode(clean).map_err(|_| invalid())
        }
        KeyEncoding::Base64 => {
            let clean: String = key.chars().filter(|c| !c.is_whitespace()).collect();
            data_encoding::BASE64
                .decode(clean.as_bytes())
                .or_else(|_| {
                    data_encoding::BASE64URL_NOPAD.decode(clean.trim_end_matches('=').as_bytes())
                })
                .map_err(|_| invalid())
        }
    }
}

/// 解析用户粘贴的期望值：依次尝试十六进制与 Base64
fn decode_expected(expected: &str) -> Option<Vec<u8>> {
    let clean: String = expected.chars().filter(|c| !c.is_whitespace()).collect();
    if clean.is_empty() {
        return None;
    }
    hex::decode(&clean)
        .ok()
        .or_else(|| data_encoding::BASE64.decode(clean.as_bytes()).ok())
        .or_else(|| {
            data_encoding::BASE64URL_NOPAD
                .decode(clean.trim_end_matches('=').as_bytes())
                .ok()
        })
}

/// 常量时间比较，避免把校验结果泄露为时间差
fn equal(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub fn hmac_text(args: Args) -> PluginResult<Report> {
    let algorithms: Vec<_> = args
        .algorithms
        .iter()
        .copied()
        .filter(|a| *a != Algorithm::Crc32)
        .collect();
    if algorithms.is_empty() {
        return Err(PluginError::new("hash.no_algorithms"));
    }
    let start = Instant::now();
    let key = decode_key(&args.key, args.key_encoding)?;
    let expected = decode_expected(&args.expected);
    let results = algorithms
        .into_iter()
        .filter_map(|algorithm| {
            let bytes = mac(algorithm, &key, args.input.as_bytes())?;
            let text = match args.output {
                OutputEncoding::Hex if args.uppercase => hex::encode_upper(&bytes),
                OutputEncoding::Hex => hex::encode(&bytes),
                OutputEncoding::Base64 => data_encoding::BASE64.encode(&bytes),
            };
            Some(Item {
                algorithm,
                mac: text,
                matches: expected.as_ref().map(|e| equal(e, &bytes)),
            })
        })
        .collect();
    Ok(Report {
        results,
        key_bytes: key.len(),
        elapsed_ms: (start.elapsed().as_secs_f64() * 100_000.0).round() / 100.0,
    })
}

#[cfg(test)]
#[path = "hmac_test.rs"]
mod tests;
