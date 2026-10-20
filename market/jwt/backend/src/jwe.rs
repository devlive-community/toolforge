//! JWE Compact 五段式解密调试（RFC 7516）：仅支持 dir + A256GCM 与
//! RSA-OAEP-256 + A256GCM 两种组合。protected header 编码段原样作为 AAD，
//! 不先改写 JSON；解密与认证全部成功才展示明文，错密钥/篡改统一失败，无部分明文。
//! alg/enc 走允许清单，crit/zip/非法组合明确拒绝；密钥与明文属敏感数据，不落日志。

use aes_gcm::Aes256Gcm;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use data_encoding::BASE64URL_NOPAD;
use rsa::Oaep;
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs8::DecodePrivateKey;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tf_plugin_api::{PluginError, PluginResult};

/// 令牌与明文大小上限
pub const MAX_TOKEN: usize = 64 << 10;
/// A256GCM 参数
const IV_LEN: usize = 12;
const TAG_LEN: usize = 16;
const CEK_LEN: usize = 32;

fn errp(code: &str, key: &str, value: impl Into<serde_json::Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

fn decrypt_error() -> PluginError {
    // 错密钥与篡改统一为认证失败，不区分细节
    PluginError::new("jwe.decrypt_failed")
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecryptArgs {
    /// JWE Compact：header.encrypted_key.iv.ciphertext.tag
    pub token: String,
    /// dir 模式：256 位原始密钥，hex 或 base64url 编码
    pub key_text: Option<String>,
    /// 密钥编码：hex | base64url
    #[serde(default)]
    pub key_encoding: Option<String>,
    /// RSA-OAEP-256 模式：PEM 私钥（PKCS#8 或 PKCS#1）
    pub private_pem: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseArgs {
    pub token: String,
}

/// 仅解析（无需密钥）：protected header、五段长度与算法组合
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseOut {
    pub header: serde_json::Value,
    pub parts: [usize; 5],
    pub alg: String,
    pub enc: String,
    pub encrypted_key_len: usize,
}

pub fn parse(args: ParseArgs) -> PluginResult<ParseOut> {
    let parts = parse_parts(&args.token)?;
    let (segment_lens, encrypted_key_len, alg, enc) = summarize(&parts);
    Ok(ParseOut {
        header: parts.header,
        parts: segment_lens,
        alg,
        enc,
        encrypted_key_len,
    })
}

fn summarize(parts: &Parts) -> ([usize; 5], usize, String, String) {
    (
        [
            parts.header_raw.chars().count(),
            parts.encrypted_key_raw.chars().count(),
            parts.iv_raw.chars().count(),
            parts.ciphertext_raw.chars().count(),
            parts.tag_raw.chars().count(),
        ],
        parts.encrypted_key_raw.chars().count(),
        parts.alg.clone(),
        parts.enc.clone(),
    )
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecryptOut {
    /// 仅解析（未认证）时可见的受保护头
    pub header: serde_json::Value,
    /// 五段长度（字符）
    pub parts: [usize; 5],
    pub alg: String,
    pub enc: String,
    /// 加密密钥段是否为空（dir 要求为空）
    pub encrypted_key_len: usize,
    /// 解密与认证成功后的明文
    pub plaintext: String,
    /// 明文解释：utf8 | json | binary
    pub kind: String,
    /// 二进制明文的 base64url 表示
    pub plaintext_base64: String,
}

/// 严格 Base64url 解码（拒绝标准 Base64 字符与填充）
fn b64url_decode(text: &str) -> PluginResult<Vec<u8>> {
    BASE64URL_NOPAD
        .decode(text.as_bytes())
        .map_err(|_| errp("jwe.bad_base64url", "value", truncate(text)))
}

fn truncate(text: &str) -> String {
    text.chars().take(24).collect()
}

/// 解析五段并校验 protected header 结构（无密钥也可查看：仅解析，未认证）
struct Parts {
    header_raw: String,
    encrypted_key_raw: String,
    iv_raw: String,
    ciphertext_raw: String,
    tag_raw: String,
    header: serde_json::Value,
    alg: String,
    enc: String,
}

fn parse_parts(token: &str) -> PluginResult<Parts> {
    let token = token.trim();
    if token.len() > MAX_TOKEN {
        return Err(errp("jwe.too_large", "max", MAX_TOKEN));
    }
    let segments: Vec<&str> = token.split('.').collect();
    if segments.len() != 5 {
        return Err(errp("jwe.bad_parts", "count", segments.len()));
    }
    let header_raw = segments[0];
    let header_bytes = b64url_decode(header_raw)?;
    if header_bytes.len() > 4096 {
        return Err(errp("jwe.header_too_large", "max", 4096));
    }
    let header: serde_json::Value = serde_json::from_slice(&header_bytes)
        .map_err(|_| PluginError::new("jwe.bad_header_json"))?;
    let alg = header
        .get("alg")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PluginError::new("jwe.missing_alg"))?
        .to_owned();
    let enc = header
        .get("enc")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PluginError::new("jwe.missing_enc"))?
        .to_owned();
    // 允许清单；crit/zip 明确拒绝
    if header.get("crit").is_some() {
        return Err(PluginError::new("jwe.crit_unsupported"));
    }
    if header.get("zip").is_some() {
        return Err(PluginError::new("jwe.zip_unsupported"));
    }
    let allowed = matches!(alg.as_str(), "dir" | "RSA-OAEP-256") && enc == "A256GCM";
    if !allowed {
        return Err(errp("jwe.unsupported_combo", "alg", alg.as_str()).with("enc", enc.as_str()));
    }
    Ok(Parts {
        header_raw: header_raw.to_owned(),
        encrypted_key_raw: segments[1].to_owned(),
        iv_raw: segments[2].to_owned(),
        ciphertext_raw: segments[3].to_owned(),
        tag_raw: segments[4].to_owned(),
        header,
        alg,
        enc,
    })
}

/// dir：CEK 即输入密钥；encrypted-key 段必须为空
fn cek_from_dir(args: &DecryptArgs, parts: &Parts) -> PluginResult<Vec<u8>> {
    if !parts.encrypted_key_raw.is_empty() {
        return Err(PluginError::new("jwe.encrypted_key_expected_empty"));
    }
    let Some(key_text) = args
        .key_text
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return Err(PluginError::new("jwe.key_required"));
    };
    let encoding = args.key_encoding.as_deref().unwrap_or("hex");
    let key = match encoding {
        "hex" => {
            let hex = key_text.replace([' ', ':', '-'], "");
            decode_hex(&hex)?
        }
        "base64url" => b64url_decode(key_text)?,
        other => return Err(errp("jwe.bad_key_encoding", "value", other)),
    };
    if key.len() != CEK_LEN {
        return Err(errp("jwe.bad_key_len", "expected", CEK_LEN).with("actual", key.len()));
    }
    Ok(key)
}

fn decode_hex(text: &str) -> PluginResult<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(PluginError::new("jwe.bad_hex"));
    }
    (0..text.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&text[i..i + 2], 16).map_err(|_| PluginError::new("jwe.bad_hex"))
        })
        .collect()
}

/// RSA-OAEP-256：PEM 私钥解出 CEK
fn cek_from_rsa(args: &DecryptArgs, encrypted_key: &[u8]) -> PluginResult<Vec<u8>> {
    let Some(pem) = args
        .private_pem
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return Err(PluginError::new("jwe.key_required"));
    };
    let private_key = rsa::RsaPrivateKey::from_pkcs8_pem(pem)
        .or_else(|_| rsa::RsaPrivateKey::from_pkcs1_pem(pem))
        .map_err(|_| PluginError::new("jwe.bad_private_key"))?;
    let decrypted = private_key
        .decrypt(Oaep::new::<Sha256>(), encrypted_key)
        .map_err(|_| decrypt_error())?;
    if decrypted.len() != CEK_LEN {
        return Err(errp("jwe.bad_cek_len", "expected", CEK_LEN).with("actual", decrypted.len()));
    }
    Ok(decrypted)
}

/// JWE 解密：解析（未认证）→ 组合允许清单校验 → CEK → A256GCM 认证解密
pub fn decrypt(args: DecryptArgs) -> PluginResult<DecryptOut> {
    let parts = parse_parts(&args.token)?;
    let (out_parts, encrypted_key_len, alg, enc) = summarize(&parts);
    let header = parts.header.clone();

    // IV / tag 长度在认证解密前检查
    let iv = b64url_decode(&parts.iv_raw)?;
    if iv.len() != IV_LEN {
        return Err(errp("jwe.bad_iv_len", "expected", IV_LEN).with("actual", iv.len()));
    }
    let tag = b64url_decode(&parts.tag_raw)?;
    if tag.len() != TAG_LEN {
        return Err(errp("jwe.bad_tag_len", "expected", TAG_LEN).with("actual", tag.len()));
    }
    let ciphertext = b64url_decode(&parts.ciphertext_raw)?;

    // CEK 按算法组合派生
    let cek = if alg == "dir" {
        cek_from_dir(&args, &parts)?
    } else {
        let encrypted_key = b64url_decode(&parts.encrypted_key_raw)?;
        cek_from_rsa(&args, &encrypted_key)?
    };

    // AAD = 原始 protected header 编码段（规范要求，不改写 JSON）
    let aad = parts.header_raw.as_bytes();
    let cipher = Aes256Gcm::new_from_slice(&cek).map_err(|_| decrypt_error())?;
    // aes-gcm 需要密文与 tag 相连
    let mut sealed = ciphertext.clone();
    sealed.extend_from_slice(&tag);
    let nonce = aes_gcm::Nonce::from_slice(&iv);
    let plaintext = cipher
        .decrypt(nonce, Payload { msg: &sealed, aad })
        .map_err(|_| decrypt_error())?;

    // 明文解释：不强行当作 JWT claims
    let (kind, text, b64) = match std::str::from_utf8(&plaintext) {
        Ok(text) => {
            let looks_json = serde_json::from_str::<serde_json::Value>(text).is_ok();
            (
                if looks_json { "json" } else { "utf8" }.to_owned(),
                text.to_owned(),
                String::new(),
            )
        }
        Err(_) => (
            "binary".to_owned(),
            String::new(),
            BASE64URL_NOPAD.encode(&plaintext),
        ),
    };

    Ok(DecryptOut {
        header,
        parts: out_parts,
        alg,
        enc,
        encrypted_key_len,
        plaintext: text,
        kind,
        plaintext_base64: b64,
    })
}

#[cfg(test)]
#[path = "jwe_test.rs"]
mod tests;
