//! 生成、解析与转换 SSH 密钥：OpenSSH 格式的公钥 / 私钥，authorized_keys 与 known_hosts 行，
//! 以及旧式 PEM 格式的 RSA 私钥。

use md5::{Digest, Md5};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use ssh_key::private::{KeypairData, RsaKeypair};
use ssh_key::public::KeyData;
use ssh_key::{Algorithm, EcdsaCurve, HashAlg, LineEnding, PrivateKey, PublicKey};
use tf_plugin_api::{PluginError, PluginResult};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Ed25519,
    Ecdsa256,
    Ecdsa384,
    Ecdsa521,
    Rsa2048,
    Rsa3072,
    Rsa4096,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    /// public / private
    pub kind: &'static str,
    /// 算法名，如 ssh-ed25519、rsa-sha2-512、ecdsa-sha2-nistp256
    pub algorithm: String,
    /// 便于展示的类型：ED25519、RSA、ECDSA
    pub label: &'static str,
    pub bits: usize,
    pub comment: String,
    pub encrypted: bool,
    pub cipher: Option<String>,
    pub sha256: String,
    pub md5: String,
    pub randomart: String,
    /// OpenSSH 单行公钥
    pub public: String,
    /// authorized_keys 行前的选项
    pub options: Option<String>,
    /// known_hosts 行的主机
    pub hosts: Option<String>,
    /// 原始输入中的行号（从 1 开始）
    pub line: usize,
    /// 旧式 PEM 私钥转换出的 OpenSSH 私钥
    pub converted: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    pub line: usize,
    pub code: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inspection {
    pub keys: Vec<Info>,
    pub problems: Vec<Problem>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Generated {
    pub private: String,
    pub public: String,
    pub info: Info,
}

fn fail(code: &str, err: impl ToString) -> PluginError {
    PluginError::new(code).with("detail", err.to_string())
}

fn label(algorithm: &Algorithm) -> &'static str {
    match algorithm {
        Algorithm::Ed25519 => "ED25519",
        Algorithm::Rsa { .. } => "RSA",
        Algorithm::Ecdsa { .. } => "ECDSA",
        Algorithm::Dsa => "DSA",
        _ => "OTHER",
    }
}

fn bits(key: &KeyData) -> usize {
    match key {
        KeyData::Ed25519(_) => 256,
        KeyData::Ecdsa(ecdsa) => match ecdsa.curve() {
            EcdsaCurve::NistP256 => 256,
            EcdsaCurve::NistP384 => 384,
            EcdsaCurve::NistP521 => 521,
        },
        KeyData::Rsa(rsa) => {
            let bytes = rsa.n.as_bytes();
            let leading = bytes.iter().take_while(|b| **b == 0).count();
            match bytes.get(leading) {
                Some(first) => (bytes.len() - leading) * 8 - first.leading_zeros() as usize,
                None => 0,
            }
        }
        _ => 0,
    }
}

/// OpenSSH 的随机图（drunken bishop）：9 行 17 列
pub fn randomart(title: &str, digest: &[u8], footer: &str) -> String {
    const WIDTH: usize = 17;
    const HEIGHT: usize = 9;
    const SYMBOLS: &[u8] = b" .o+=*BOX@%&#/^SE";
    let mut field = [[0u8; WIDTH]; HEIGHT];
    let (mut x, mut y) = (WIDTH / 2, HEIGHT / 2);
    let (start_x, start_y) = (x, y);
    for byte in digest {
        let mut input = *byte;
        for _ in 0..4 {
            x = if input & 1 == 1 {
                (x + 1).min(WIDTH - 1)
            } else {
                x.saturating_sub(1)
            };
            y = if input & 2 == 2 {
                (y + 1).min(HEIGHT - 1)
            } else {
                y.saturating_sub(1)
            };
            let cell = &mut field[y][x];
            if (*cell as usize) < SYMBOLS.len() - 3 {
                *cell += 1;
            }
            input >>= 2;
        }
    }
    let frame = |text: &str| {
        let text = format!("[{text}]");
        let pad = WIDTH.saturating_sub(text.len());
        let left = pad / 2;
        format!("+{}{}{}+", "-".repeat(left), text, "-".repeat(pad - left))
    };
    let mut out = frame(title);
    out.push('\n');
    for (row, cells) in field.iter().enumerate() {
        out.push('|');
        for (column, cell) in cells.iter().enumerate() {
            let symbol = if (column, row) == (start_x, start_y) {
                b'S'
            } else if (column, row) == (x, y) {
                b'E'
            } else {
                SYMBOLS[*cell as usize]
            };
            out.push(symbol as char);
        }
        out.push_str("|\n");
    }
    out.push_str(&frame(footer));
    out
}

fn describe(
    public: &PublicKey,
    kind: &'static str,
    encrypted: bool,
    cipher: Option<String>,
    line: usize,
) -> PluginResult<Info> {
    let sha256 = public.fingerprint(HashAlg::Sha256);
    let blob = public.to_bytes().map_err(|e| fail("ssh.invalid_key", e))?;
    let md5 = Md5::digest(&blob)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(":");
    let algorithm = public.algorithm();
    let bits = bits(public.key_data());
    let title = format!("{} {}", label(&algorithm), bits);
    let mut plain = public.clone();
    plain.set_comment("");
    Ok(Info {
        kind,
        algorithm: algorithm.as_str().to_owned(),
        label: label(&algorithm),
        bits,
        comment: public.comment().to_owned(),
        encrypted,
        cipher,
        randomart: randomart(&title, sha256.as_bytes(), "SHA256"),
        sha256: sha256.to_string(),
        md5: format!("MD5:{md5}"),
        public: public
            .to_openssh()
            .map_err(|e| fail("ssh.invalid_key", e))?,
        options: None,
        hosts: None,
        line,
        converted: None,
    })
}

pub fn generate(kind: Kind, comment: &str, passphrase: Option<&str>) -> PluginResult<Generated> {
    let mut rng = OsRng;
    let mut key = match kind {
        Kind::Ed25519 => PrivateKey::random(&mut rng, Algorithm::Ed25519),
        Kind::Ecdsa256 => PrivateKey::random(
            &mut rng,
            Algorithm::Ecdsa {
                curve: EcdsaCurve::NistP256,
            },
        ),
        Kind::Ecdsa384 => PrivateKey::random(
            &mut rng,
            Algorithm::Ecdsa {
                curve: EcdsaCurve::NistP384,
            },
        ),
        Kind::Ecdsa521 => PrivateKey::random(
            &mut rng,
            Algorithm::Ecdsa {
                curve: EcdsaCurve::NistP521,
            },
        ),
        Kind::Rsa2048 | Kind::Rsa3072 | Kind::Rsa4096 => {
            let size = match kind {
                Kind::Rsa2048 => 2048,
                Kind::Rsa3072 => 3072,
                _ => 4096,
            };
            RsaKeypair::random(&mut rng, size)
                .and_then(|pair| PrivateKey::new(KeypairData::from(pair), ""))
        }
    }
    .map_err(|e| fail("ssh.generate_failed", e))?;
    key.set_comment(comment.trim());
    let public = key.public_key().clone();
    let key = match passphrase.filter(|p| !p.is_empty()) {
        Some(passphrase) => key
            .encrypt(&mut rng, passphrase)
            .map_err(|e| fail("ssh.encrypt_failed", e))?,
        None => key,
    };
    let private = key
        .to_openssh(LineEnding::LF)
        .map_err(|e| fail("ssh.generate_failed", e))?
        .to_string();
    let info = describe(
        &public,
        "private",
        key.is_encrypted(),
        key.is_encrypted().then(|| key.cipher().as_str().to_owned()),
        1,
    )?;
    Ok(Generated {
        public: info.public.clone(),
        private,
        info,
    })
}

const ALGORITHMS: [&str; 8] = [
    "ssh-ed25519",
    "ssh-rsa",
    "rsa-sha2-256",
    "rsa-sha2-512",
    "ecdsa-sha2-nistp256",
    "ecdsa-sha2-nistp384",
    "ecdsa-sha2-nistp521",
    "sk-ssh-ed25519@openssh.com",
];

/// 拆出公钥前面的 authorized_keys 选项或 known_hosts 主机；选项中的引号内可能有空格
fn split_prefix(line: &str) -> (Option<&str>, &str) {
    let mut in_quotes = false;
    let mut start = 0;
    for (index, ch) in line.char_indices() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ' ' | '\t' if !in_quotes => {
                let token = &line[start..index];
                if ALGORITHMS.contains(&token) {
                    let prefix = line[..start].trim();
                    return ((!prefix.is_empty()).then_some(prefix), &line[start..]);
                }
                start = index + 1;
            }
            _ => {}
        }
    }
    (None, line)
}

fn inspect_public(line: &str, number: usize) -> PluginResult<Info> {
    let (prefix, key) = split_prefix(line);
    let public =
        PublicKey::from_openssh(key.trim()).map_err(|_| PluginError::new("ssh.invalid_key"))?;
    let mut info = describe(&public, "public", false, None, number)?;
    if let Some(prefix) = prefix {
        // known_hosts 的主机字段是逗号分隔的主机名 / 哈希；authorized_keys 选项带 = 或关键字
        let looks_like_hosts = prefix.starts_with('|')
            || prefix.starts_with('[')
            || prefix.starts_with('@')
            || (!prefix.contains('=')
                && !prefix.contains('"')
                && prefix.chars().any(|c| c == '.' || c == ':'));
        if looks_like_hosts {
            info.hosts = Some(prefix.to_owned());
        } else {
            info.options = Some(prefix.to_owned());
        }
    }
    Ok(info)
}

fn inspect_private(pem: &str, passphrase: Option<&str>, line: usize) -> PluginResult<Info> {
    if pem.contains("BEGIN RSA PRIVATE KEY") || pem.contains("BEGIN PRIVATE KEY") {
        return inspect_legacy_rsa(pem, line);
    }
    if pem.contains("BEGIN ENCRYPTED PRIVATE KEY")
        || (pem.contains("Proc-Type") && pem.contains("ENCRYPTED"))
    {
        return Err(PluginError::new("ssh.legacy_encrypted"));
    }
    let key = PrivateKey::from_openssh(pem).map_err(|e| fail("ssh.invalid_private_key", e))?;
    let cipher = key.is_encrypted().then(|| key.cipher().as_str().to_owned());
    // 加密的私钥也能直接读取公钥部分；提供了口令时顺便校验
    if let Some(passphrase) = passphrase.filter(|p| !p.is_empty())
        && key.is_encrypted()
    {
        key.decrypt(passphrase)
            .map_err(|_| PluginError::new("ssh.wrong_passphrase"))?;
    }
    describe(
        key.public_key(),
        "private",
        key.is_encrypted(),
        cipher,
        line,
    )
}

/// PKCS#1 / PKCS#8 格式的 RSA 私钥，转换为 OpenSSH 格式
fn inspect_legacy_rsa(pem: &str, line: usize) -> PluginResult<Info> {
    use rsa::pkcs1::DecodeRsaPrivateKey;
    use rsa::pkcs8::DecodePrivateKey;
    let rsa = rsa::RsaPrivateKey::from_pkcs1_pem(pem)
        .or_else(|_| rsa::RsaPrivateKey::from_pkcs8_pem(pem))
        .map_err(|_| PluginError::new("ssh.unsupported_pem"))?;
    let pair = RsaKeypair::try_from(rsa).map_err(|e| fail("ssh.invalid_private_key", e))?;
    let key = PrivateKey::new(KeypairData::from(pair), "")
        .map_err(|e| fail("ssh.invalid_private_key", e))?;
    let mut info = describe(key.public_key(), "private", false, None, line)?;
    info.converted = Some(
        key.to_openssh(LineEnding::LF)
            .map_err(|e| fail("ssh.invalid_private_key", e))?
            .to_string(),
    );
    Ok(info)
}

/// 解析粘贴的文本：可包含多个公钥行（authorized_keys、known_hosts）与私钥块
pub fn inspect(text: &str, passphrase: Option<&str>) -> Inspection {
    let mut keys = Vec::new();
    let mut problems = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].trim();
        let number = index + 1;
        if line.starts_with("-----BEGIN ") && line.contains("PRIVATE KEY") {
            let end = lines[index..]
                .iter()
                .position(|l| l.trim().starts_with("-----END "))
                .map_or(lines.len(), |p| index + p + 1);
            let block = lines[index..end].join("\n");
            match inspect_private(&block, passphrase, number) {
                Ok(info) => keys.push(info),
                Err(err) => problems.push(Problem {
                    line: number,
                    code: err.code,
                }),
            }
            index = end;
            continue;
        }
        if !line.is_empty() && !line.starts_with('#') {
            match inspect_public(line, number) {
                Ok(info) => keys.push(info),
                Err(err) => problems.push(Problem {
                    line: number,
                    code: err.code,
                }),
            }
        }
        index += 1;
    }
    Inspection { keys, problems }
}

/// 修改私钥口令；新口令为空时移除加密
pub fn change_passphrase(pem: &str, old: Option<&str>, new: Option<&str>) -> PluginResult<String> {
    let key =
        PrivateKey::from_openssh(pem.trim()).map_err(|e| fail("ssh.invalid_private_key", e))?;
    let key = if key.is_encrypted() {
        let old = old
            .filter(|p| !p.is_empty())
            .ok_or_else(|| PluginError::new("ssh.passphrase_required"))?;
        key.decrypt(old)
            .map_err(|_| PluginError::new("ssh.wrong_passphrase"))?
    } else {
        key
    };
    let key = match new.filter(|p| !p.is_empty()) {
        Some(new) => key
            .encrypt(&mut OsRng, new)
            .map_err(|e| fail("ssh.encrypt_failed", e))?,
        None => key,
    };
    Ok(key
        .to_openssh(LineEnding::LF)
        .map_err(|e| fail("ssh.invalid_private_key", e))?
        .to_string())
}

#[cfg(test)]
#[path = "keys_test.rs"]
mod tests;
