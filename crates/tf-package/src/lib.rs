//! 插件包（.tfp）：zip 格式，包含 manifest、WASM 组件、界面模块与语言包，可带 ed25519 签名。
//!
//! 包内文件：
//! - `manifest.json`、`plugin.wasm`、`ui/index.js`（必需）
//! - `ui/index.css`、`icon.svg`、`locales/<语言>.json`（可选）
//! - `SIGNATURE`（可选）：`{"keyId": "...", "signature": "<base64>"}`
//!
//! 签名对象是内容摘要：除 SIGNATURE 外所有文件按路径排序，逐行写入 `<sha256>  <路径>\n` 后取 SHA-256。

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

use data_encoding::{BASE64, HEXLOWER};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tf_plugin_api::Manifest;

/// 包格式错误：错误码 + 参数，由宿主翻译
#[derive(Debug, Clone, PartialEq)]
pub struct PackageError {
    pub code: &'static str,
    pub params: Vec<(&'static str, String)>,
}

impl PackageError {
    pub fn new(code: &'static str) -> Self {
        Self {
            code,
            params: Vec::new(),
        }
    }

    pub fn with(mut self, key: &'static str, value: impl ToString) -> Self {
        self.params.push((key, value.to_string()));
        self
    }
}

impl std::fmt::Display for PackageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code)?;
        for (key, value) in &self.params {
            write!(f, " {key}={value}")?;
        }
        Ok(())
    }
}

impl std::error::Error for PackageError {}

pub type PackageResult<T> = Result<T, PackageError>;

pub const MAX_PACKAGE: usize = 64 * 1024 * 1024;
pub const MAX_UNPACKED: u64 = 256 * 1024 * 1024;
const MAX_ENTRIES: usize = 512;
pub const SIGNATURE_FILE: &str = "SIGNATURE";
const REQUIRED: [&str; 3] = ["manifest.json", "plugin.wasm", "ui/index.js"];

/// 官方签名公钥：(keyId, 公钥)
pub const OFFICIAL_KEYS: &[(&str, &str)] = &[(
    "toolforge-2026",
    "STmFefypDab325LKaHp2/sAEgbcsf13uwB3bzva/xgY=",
)];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SignatureFile {
    pub key_id: String,
    pub signature: String,
}

/// 签名校验结果
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Trust {
    /// 由官方密钥签名
    Official { key_id: String },
    /// 没有签名（只有用户确认后才能安装）
    Unsigned,
    /// 带签名但密钥不是官方密钥（视同未签名）
    UnknownKey { key_id: String },
}

#[derive(Debug)]
pub struct Package {
    pub manifest: Manifest,
    pub files: BTreeMap<String, Vec<u8>>,
    /// 内容摘要（十六进制）
    pub digest: String,
    pub trust: Trust,
}

fn invalid(detail: impl ToString) -> PackageError {
    PackageError::new("package.invalid").with("detail", detail.to_string())
}

/// 包内路径只允许 manifest、组件、ui/ 与 locales/ 下的普通相对路径
fn allowed(path: &str) -> bool {
    let safe = !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..");
    safe && (REQUIRED.contains(&path)
        || path == SIGNATURE_FILE
        || path == "icon.svg"
        || path.starts_with("ui/")
        || (path.starts_with("locales/")
            && path.ends_with(".json")
            && path.matches('/').count() == 1))
}

pub fn digest(files: &BTreeMap<String, Vec<u8>>) -> String {
    let mut listing = String::new();
    for (path, data) in files.iter().filter(|(p, _)| p.as_str() != SIGNATURE_FILE) {
        listing.push_str(&HEXLOWER.encode(&Sha256::digest(data)));
        listing.push_str("  ");
        listing.push_str(path);
        listing.push('\n');
    }
    HEXLOWER.encode(&Sha256::digest(listing.as_bytes()))
}

/// 插件 id：小写字母、数字、点与连字符，2–128 个字符
pub fn valid_id(id: &str) -> bool {
    (2..=128).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
        && !id.starts_with('.')
        && !id.contains("..")
}

fn verify(digest: &str, signature: &SignatureFile, keys: &[(&str, &str)]) -> PackageResult<Trust> {
    let Some((_, key)) = keys.iter().find(|(id, _)| *id == signature.key_id) else {
        return Ok(Trust::UnknownKey {
            key_id: signature.key_id.clone(),
        });
    };
    let key: [u8; 32] = BASE64
        .decode(key.as_bytes())
        .ok()
        .and_then(|k| k.try_into().ok())
        .ok_or_else(|| PackageError::new("package.bad_key"))?;
    let key = VerifyingKey::from_bytes(&key).map_err(|_| PackageError::new("package.bad_key"))?;
    let sig: [u8; 64] = BASE64
        .decode(signature.signature.as_bytes())
        .ok()
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| PackageError::new("package.bad_signature"))?;
    key.verify(digest.as_bytes(), &Signature::from_bytes(&sig))
        .map_err(|_| PackageError::new("package.bad_signature"))?;
    Ok(Trust::Official {
        key_id: signature.key_id.clone(),
    })
}

/// 读取并校验插件包；签名存在但不正确时直接拒绝
pub fn open(bytes: &[u8]) -> PackageResult<Package> {
    open_with_keys(bytes, OFFICIAL_KEYS)
}

pub fn open_with_keys(bytes: &[u8], keys: &[(&str, &str)]) -> PackageResult<Package> {
    if bytes.len() > MAX_PACKAGE {
        return Err(PackageError::new("package.too_large").with("mb", 64));
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(invalid)?;
    if archive.len() > MAX_ENTRIES {
        return Err(invalid("too many entries"));
    }
    let mut files = BTreeMap::new();
    let mut total = 0u64;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(invalid)?;
        if entry.is_dir() {
            continue;
        }
        let path = entry.name().to_owned();
        if !allowed(&path) {
            return Err(PackageError::new("package.bad_path").with("path", path));
        }
        total += entry.size();
        if total > MAX_UNPACKED {
            return Err(PackageError::new("package.too_large").with("mb", 256));
        }
        let mut data = Vec::with_capacity(entry.size() as usize);
        (&mut entry)
            .take(MAX_UNPACKED)
            .read_to_end(&mut data)
            .map_err(invalid)?;
        if files.insert(path.clone(), data).is_some() {
            return Err(invalid(format!("duplicate entry {path}")));
        }
    }
    for required in REQUIRED {
        if !files.contains_key(required) {
            return Err(PackageError::new("package.missing_file").with("path", required));
        }
    }
    let manifest: Manifest = serde_json::from_slice(&files["manifest.json"])
        .map_err(|e| invalid(format!("manifest: {e}")))?;
    if !valid_id(&manifest.id) {
        return Err(PackageError::new("package.bad_id").with("id", manifest.id));
    }
    let digest = digest(&files);
    let trust = match files.get(SIGNATURE_FILE) {
        None => Trust::Unsigned,
        Some(raw) => {
            let signature: SignatureFile = serde_json::from_slice(raw)
                .map_err(|_| PackageError::new("package.bad_signature"))?;
            verify(&digest, &signature, keys)?
        }
    };
    Ok(Package {
        manifest,
        files,
        digest,
        trust,
    })
}

/// 打包（供 xtask 与测试使用）：传入私钥时附带签名
pub fn build(
    files: &BTreeMap<String, Vec<u8>>,
    signer: Option<(&str, &SigningKey)>,
) -> PackageResult<Vec<u8>> {
    let mut files = files.clone();
    files.remove(SIGNATURE_FILE);
    if let Some((key_id, key)) = signer {
        let signature = key.sign(digest(&files).as_bytes());
        let file = SignatureFile {
            key_id: key_id.to_owned(),
            signature: BASE64.encode(&signature.to_bytes()),
        };
        files.insert(
            SIGNATURE_FILE.to_owned(),
            serde_json::to_vec_pretty(&file).map_err(invalid)?,
        );
    }
    let mut out = Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(&mut out);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default());
    for (path, data) in &files {
        writer.start_file(path, options).map_err(invalid)?;
        writer.write_all(data).map_err(invalid)?;
    }
    writer.finish().map_err(invalid)?;
    Ok(out.into_inner())
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
