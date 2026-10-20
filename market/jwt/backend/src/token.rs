use std::str::FromStr;

use data_encoding::{BASE64URL, BASE64URL_NOPAD};
use jsonwebtoken::{Algorithm, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tf_plugin_api::{PluginError, PluginResult};

use crate::claims::{self, Status};
use crate::keys::{self, SecretEncoding};

#[derive(Deserialize)]
pub struct DecodeArgs {
    token: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyArgs {
    token: String,
    key: String,
    #[serde(default)]
    key_encoding: SecretEncoding,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignArgs {
    algorithm: String,
    /// 额外的头部字段（如 kid），JSON 对象文本；可为空
    #[serde(default)]
    header: String,
    payload: String,
    key: String,
    #[serde(default)]
    key_encoding: SecretEncoding,
}

/// 三段在原始令牌中的 UTF-16 区间，供前端着色
#[derive(Debug, Serialize, PartialEq)]
pub struct Segments {
    pub header: (usize, usize),
    pub payload: (usize, usize),
    pub signature: (usize, usize),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Decoded {
    pub algorithm: Option<String>,
    pub header: String,
    pub payload: String,
    pub signature: String,
    pub segments: Segments,
    pub status: Status,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Verified {
    pub valid: bool,
    pub algorithm: String,
    /// 签名无效时的原因码
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Signed {
    pub token: String,
}

/// 去掉 Bearer 前缀、首尾空白与引号
fn clean(token: &str) -> &str {
    let token = token.trim().trim_matches('"');
    let lower = token.get(..7).map(str::to_ascii_lowercase);
    if lower.as_deref() == Some("bearer ") {
        token[7..].trim()
    } else {
        token
    }
}

fn b64(part: &str, name: &str) -> PluginResult<Vec<u8>> {
    BASE64URL_NOPAD
        .decode(part.trim_end_matches('=').as_bytes())
        .or_else(|_| BASE64URL.decode(part.as_bytes()))
        .map_err(|_| PluginError::new("jwt.invalid_base64").with("part", name))
}

fn json(bytes: &[u8], name: &str) -> PluginResult<Value> {
    serde_json::from_slice(bytes).map_err(|e| {
        PluginError::new("jwt.invalid_json")
            .with("part", name)
            .with("detail", e.to_string())
    })
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}

fn split(token: &str) -> PluginResult<(&str, &str, &str)> {
    let mut parts = token.split('.');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(h), Some(p), Some(s), None) if !h.is_empty() && !p.is_empty() => Ok((h, p, s)),
        _ => Err(PluginError::new("jwt.malformed")),
    }
}

pub fn decode(args: DecodeArgs) -> PluginResult<Decoded> {
    let raw = args.token.as_str();
    let token = clean(raw);
    if token.is_empty() {
        return Err(PluginError::new("jwt.empty"));
    }
    let (h, p, s) = split(token)?;
    let header = json(&b64(h, "header")?, "header")?;
    let payload = json(&b64(p, "payload")?, "payload")?;
    if !header.is_object() {
        return Err(PluginError::new("jwt.invalid_json").with("part", "header"));
    }

    // 令牌在原始输入中的起点（跳过 Bearer 等前缀），换算为 UTF-16 偏移
    let offset = raw
        .find(token)
        .map(|i| raw[..i].encode_utf16().count())
        .unwrap_or(0);
    let (hl, pl, sl) = (h.len(), p.len(), s.len());
    let segments = Segments {
        header: (offset, offset + hl),
        payload: (offset + hl + 1, offset + hl + 1 + pl),
        signature: (offset + hl + pl + 2, offset + hl + pl + 2 + sl),
    };

    Ok(Decoded {
        algorithm: header.get("alg").and_then(Value::as_str).map(str::to_owned),
        header: pretty(&header),
        status: claims::status(&payload, jiff::Timestamp::now().as_second()),
        payload: pretty(&payload),
        signature: s.to_owned(),
        segments,
    })
}

fn algorithm(name: &str) -> PluginResult<Algorithm> {
    if name.eq_ignore_ascii_case("none") {
        return Err(PluginError::new("jwt.none_alg"));
    }
    Algorithm::from_str(name).map_err(|_| PluginError::new("jwt.unsupported_alg").with("alg", name))
}

/// 只校验签名；过期等时间状态由 decode 单独给出
pub fn verify(args: VerifyArgs) -> PluginResult<Verified> {
    let token = clean(&args.token);
    let (h, _, _) = split(token)?;
    let header = json(&b64(h, "header")?, "header")?;
    let name = header
        .get("alg")
        .and_then(Value::as_str)
        .unwrap_or("none")
        .to_owned();
    let alg = algorithm(&name)?;
    let key = keys::decoding(alg, &args.key, args.key_encoding)?;

    let mut validation = Validation::new(alg);
    validation.validate_exp = false;
    validation.validate_nbf = false;
    validation.validate_aud = false;
    validation.required_spec_claims.clear();

    match jsonwebtoken::decode::<Value>(token, &key, &validation) {
        Ok(_) => Ok(Verified {
            valid: true,
            algorithm: name,
            reason: None,
        }),
        Err(err) => {
            use jsonwebtoken::errors::ErrorKind;
            let reason = match err.kind() {
                ErrorKind::InvalidSignature => "jwt.bad_signature",
                ErrorKind::InvalidAlgorithm => "jwt.algorithm_mismatch",
                ErrorKind::InvalidKeyFormat
                | ErrorKind::InvalidRsaKey(_)
                | ErrorKind::InvalidEcdsaKey => "jwt.invalid_key",
                _ => "jwt.verify_failed",
            };
            Ok(Verified {
                valid: false,
                algorithm: name,
                reason: Some(reason.to_owned()),
            })
        }
    }
}

pub fn sign(args: SignArgs) -> PluginResult<Signed> {
    let alg = algorithm(&args.algorithm)?;
    let payload = json(args.payload.trim().as_bytes(), "payload")?;
    if !payload.is_object() {
        return Err(PluginError::new("jwt.payload_not_object"));
    }
    let mut header = Header::new(alg);
    if !args.header.trim().is_empty() {
        let extra: Map<String, Value> = serde_json::from_str(args.header.trim()).map_err(|e| {
            PluginError::new("jwt.invalid_json")
                .with("part", "header")
                .with("detail", e.to_string())
        })?;
        if let Some(kid) = extra.get("kid").and_then(Value::as_str) {
            header.kid = Some(kid.to_owned());
        }
        if let Some(typ) = extra.get("typ").and_then(Value::as_str) {
            header.typ = Some(typ.to_owned());
        }
        if let Some(cty) = extra.get("cty").and_then(Value::as_str) {
            header.cty = Some(cty.to_owned());
        }
    }
    let key = keys::encoding(alg, &args.key, args.key_encoding)?;
    let token = jsonwebtoken::encode(&header, &payload, &key)
        .map_err(|e| PluginError::new("jwt.sign_failed").with("detail", e.to_string()))?;
    Ok(Signed { token })
}

#[cfg(test)]
#[path = "token_test.rs"]
mod tests;
