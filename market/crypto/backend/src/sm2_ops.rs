//! SM2 签名与验签调试（GB/T 32918.2-2016，SM2-with-SM3）。
//!
//! ZA 与 e = SM3(ZA ‖ M) 在这里按字节计算，所以用户标识可以是任意字节；
//! 签名与验签的曲线运算交给 sm2 crate 的 prehash 接口（签名使用 RFC 6979 确定性 k）。
//! 签名表示支持 ASN.1 DER 与固定 64 字节的 r ‖ s，两者都严格解析。

use serde::{Deserialize, Serialize};
use sm2::dsa::signature::hazmat::{PrehashSigner, PrehashVerifier};
use sm2::dsa::{Signature, SigningKey, VerifyingKey};
use sm2::elliptic_curve::sec1::ToEncodedPoint;
use sm2::pkcs8::der::{Decode, pem};
use sm2::pkcs8::spki::{ObjectIdentifier, SubjectPublicKeyInfoRef};
use sm2::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding, PrivateKeyInfo};
use sm2::{PublicKey, SecretKey};
use sm3::{Digest, Sm3};
use tf_plugin_api::{PluginError, PluginResult};

use crate::codec::{Encoding, decode, encode, hex};

/// GB/T 32918.5 推荐曲线参数（参与 ZA 计算）
const CURVE_A: &str = "fffffffeffffffffffffffffffffffffffffffff00000000fffffffffffffffc";
const CURVE_B: &str = "28e9fa9e9d9f5e344d5a9e4bcf6509a7f39789f515ab8f92ddbcbd414d940e93";
const GX: &str = "32c4ae2c1f1981195f9904466a39c9948fe30bbff2660be1715a4589334c74c7";
const GY: &str = "bc3736a2f4f6779c59bdcee36b692153d0a9877cc62a474002df32e52139f0a0";
/// 曲线阶 n
const ORDER: &str = "fffffffeffffffffffffffffffffffff7203df6b21c6052b53bbf40939d54123";

/// GB/T 32918 附录与常见实现采用的默认用户标识
pub const DEFAULT_USER_ID: &str = "1234567812345678";

const ID_EC_PUBLIC_KEY: &str = "1.2.840.10045.2.1";
const SM2_CURVE: &str = "1.2.156.10197.1.301";

/// 已知的其他曲线与算法，用于给出「不是 SM2」的具体原因
const KNOWN_OIDS: &[(&str, &str)] = &[
    ("1.2.840.10045.3.1.7", "P-256"),
    ("1.3.132.0.34", "P-384"),
    ("1.3.132.0.35", "P-521"),
    ("1.3.132.0.10", "secp256k1"),
    ("1.3.36.3.3.2.8.1.1.7", "brainpoolP256r1"),
    ("1.2.840.113549.1.1.1", "RSA"),
    ("1.3.101.112", "Ed25519"),
    ("1.3.101.110", "X25519"),
];

fn unhex(text: &str) -> Vec<u8> {
    data_encoding::HEXLOWER
        .decode(text.as_bytes())
        .expect("常量是合法的十六进制")
}

fn order() -> [u8; 32] {
    unhex(ORDER).try_into().expect("阶为 32 字节")
}

fn oid_name(oid: &ObjectIdentifier) -> String {
    let text = oid.to_string();
    KNOWN_OIDS
        .iter()
        .find(|(o, _)| *o == text)
        .map(|(_, name)| (*name).to_owned())
        .unwrap_or(text)
}

// ---------- 密钥 ----------

/// 解析得到的 SM2 密钥；`format` 说明它来自哪种表示
#[derive(Debug)]
pub struct Key {
    pub secret: Option<SecretKey>,
    pub public: PublicKey,
    pub format: &'static str,
}

fn invalid_key(detail: impl std::fmt::Display) -> PluginError {
    PluginError::new("crypto.sm2_invalid_key").with("detail", detail.to_string())
}

/// SM2 私钥 d 必须在 [1, n-2]：签名要用到 (1 + d)⁻¹
fn secret_from_scalar(bytes: &[u8]) -> PluginResult<SecretKey> {
    let mut n_minus_1 = order();
    n_minus_1[31] -= 1;
    if bytes == n_minus_1 {
        return Err(PluginError::new("crypto.sm2_key_range"));
    }
    SecretKey::from_slice(bytes).map_err(|_| PluginError::new("crypto.sm2_key_range"))
}

fn public_from_sec1(bytes: &[u8]) -> PluginResult<PublicKey> {
    PublicKey::from_sec1_bytes(bytes).map_err(|_| PluginError::new("crypto.sm2_point_invalid"))
}

/// 检查 AlgorithmIdentifier：接受 id-ecPublicKey + SM2 曲线，以及直接以 SM2 OID 标识的写法
fn check_algorithm(
    oid: ObjectIdentifier,
    parameters: Option<ObjectIdentifier>,
) -> PluginResult<()> {
    let sm2: ObjectIdentifier = SM2_CURVE.parse().expect("常量 OID");
    if oid == sm2 {
        return Ok(());
    }
    let ec: ObjectIdentifier = ID_EC_PUBLIC_KEY.parse().expect("常量 OID");
    if oid != ec {
        return Err(PluginError::new("crypto.sm2_not_ec").with("algorithm", oid_name(&oid)));
    }
    match parameters {
        Some(curve) if curve == sm2 => Ok(()),
        Some(curve) => {
            Err(PluginError::new("crypto.sm2_wrong_curve").with("curve", oid_name(&curve)))
        }
        None => Err(PluginError::new("crypto.sm2_wrong_curve").with("curve", "-")),
    }
}

/// ECPrivateKey（RFC 5915）：外层 AlgorithmIdentifier 已确认是 SM2，这里取私钥标量
fn sec1_scalar(der: &[u8]) -> PluginResult<SecretKey> {
    let secret = SecretKey::from_sec1_der(der).map_err(invalid_key)?;
    secret_from_scalar(&secret.to_bytes())
}

fn from_pkcs8(der: &[u8]) -> PluginResult<SecretKey> {
    let info = PrivateKeyInfo::from_der(der).map_err(invalid_key)?;
    check_algorithm(info.algorithm.oid, info.algorithm.parameters_oid().ok())?;
    sec1_scalar(info.private_key)
}

fn from_spki(der: &[u8]) -> PluginResult<PublicKey> {
    let info = SubjectPublicKeyInfoRef::from_der(der).map_err(invalid_key)?;
    check_algorithm(info.algorithm.oid, info.algorithm.parameters_oid().ok())?;
    let bytes = info
        .subject_public_key
        .as_bytes()
        .ok_or_else(|| invalid_key("bit string"))?;
    public_from_sec1(bytes)
}

fn private_key(secret: SecretKey, format: &'static str) -> Key {
    Key {
        public: secret.public_key(),
        secret: Some(secret),
        format,
    }
}

fn public_key(public: PublicKey, format: &'static str) -> Key {
    Key {
        secret: None,
        public,
        format,
    }
}

fn from_pem(text: &str) -> PluginResult<Key> {
    let (label, der) = pem::decode_vec(text.as_bytes()).map_err(invalid_key)?;
    match label {
        "PRIVATE KEY" => Ok(private_key(from_pkcs8(&der)?, "pkcs8-pem")),
        "PUBLIC KEY" => Ok(public_key(from_spki(&der)?, "spki-pem")),
        "ENCRYPTED PRIVATE KEY" => Err(PluginError::new("crypto.encrypted_key")),
        other => Err(PluginError::new("crypto.sm2_unsupported_format").with("format", other)),
    }
}

fn from_der(der: &[u8]) -> PluginResult<Key> {
    if PrivateKeyInfo::from_der(der).is_ok() {
        return Ok(private_key(from_pkcs8(der)?, "pkcs8-der"));
    }
    if SubjectPublicKeyInfoRef::from_der(der).is_ok() {
        return Ok(public_key(from_spki(der)?, "spki-der"));
    }
    Err(PluginError::new("crypto.sm2_unsupported_format").with("format", "DER"))
}

/// 根据长度识别原始十六进制：32 字节私钥；65 / 33 字节 SEC1 公钥；64 字节 x ‖ y 公钥
fn from_raw(bytes: &[u8]) -> Option<PluginResult<Key>> {
    let key = match (bytes.len(), bytes.first()) {
        (32, _) => secret_from_scalar(bytes).map(|s| private_key(s, "raw-private")),
        (65, Some(0x04)) => public_from_sec1(bytes).map(|p| public_key(p, "raw-public")),
        (33, Some(0x02 | 0x03)) => {
            public_from_sec1(bytes).map(|p| public_key(p, "raw-public-compressed"))
        }
        (64, _) => {
            let mut sec1 = vec![0x04];
            sec1.extend_from_slice(bytes);
            public_from_sec1(&sec1).map(|p| public_key(p, "raw-public-xy"))
        }
        _ => return None,
    };
    Some(key)
}

/// 解析 SM2 密钥：PEM（PKCS#8 / SPKI）、十六进制或 Base64 的 DER、原始十六进制
pub fn parse_key(text: &str) -> PluginResult<Key> {
    let text = text.trim();
    if text.is_empty() {
        return Err(PluginError::new("crypto.sm2_key_required"));
    }
    if text.starts_with("-----BEGIN") {
        return from_pem(text);
    }
    if let Ok(bytes) = decode(text, Encoding::Hex, "key") {
        if let Some(key) = from_raw(&bytes) {
            return key;
        }
        return from_der(&bytes);
    }
    let bytes = decode(text, Encoding::Base64, "key")
        .map_err(|_| PluginError::new("crypto.sm2_unsupported_format").with("format", "text"))?;
    from_der(&bytes)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyInfo {
    pub kind: &'static str,
    pub format: &'static str,
    /// 未压缩公钥 04 ‖ x ‖ y
    pub public_key: String,
    pub public_key_pem: String,
}

fn spki_pem(public: &PublicKey) -> PluginResult<String> {
    public
        .to_public_key_pem(LineEnding::LF)
        .map_err(invalid_key)
}

fn public_hex(public: &PublicKey) -> String {
    hex(public.to_encoded_point(false).as_bytes())
}

pub fn inspect(text: &str) -> PluginResult<KeyInfo> {
    let key = parse_key(text)?;
    Ok(KeyInfo {
        kind: if key.secret.is_some() {
            "private"
        } else {
            "public"
        },
        format: key.format,
        public_key: public_hex(&key.public),
        public_key_pem: spki_pem(&key.public)?,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyPair {
    pub private_key: String,
    pub public_key: String,
}

fn pkcs8_pem(secret: &SecretKey) -> PluginResult<String> {
    Ok(secret
        .to_pkcs8_pem(LineEnding::LF)
        .map_err(invalid_key)?
        .to_string())
}

pub fn generate() -> PluginResult<KeyPair> {
    let secret = loop {
        let candidate = SecretKey::random(&mut rand::rngs::OsRng);
        // d = n-1 的概率可以忽略，仍按标准排除
        if secret_from_scalar(&candidate.to_bytes()).is_ok() {
            break candidate;
        }
    };
    Ok(KeyPair {
        private_key: pkcs8_pem(&secret)?,
        public_key: spki_pem(&secret.public_key())?,
    })
}

/// 把任意格式的私钥导出为 PKCS#8 PEM（只在用户显式导出时调用）
pub fn export_private(text: &str) -> PluginResult<String> {
    let key = parse_key(text)?;
    let secret = key
        .secret
        .ok_or_else(|| PluginError::new("crypto.sm2_private_required"))?;
    pkcs8_pem(&secret)
}

// ---------- ZA 与消息摘要 ----------

/// ZA = SM3(ENTLA ‖ IDA ‖ a ‖ b ‖ xG ‖ yG ‖ xA ‖ yA)，ENTLA 为 IDA 的比特长度（2 字节大端）
pub fn za(user_id: &[u8], public: &PublicKey) -> PluginResult<[u8; 32]> {
    let bits = user_id
        .len()
        .checked_mul(8)
        .and_then(|b| u16::try_from(b).ok())
        .ok_or_else(|| {
            PluginError::new("crypto.sm2_user_id_too_long").with("bytes", user_id.len())
        })?;
    let point = public.to_encoded_point(false);
    let mut hasher = Sm3::new();
    hasher.update(bits.to_be_bytes());
    hasher.update(user_id);
    for constant in [CURVE_A, CURVE_B, GX, GY] {
        hasher.update(unhex(constant));
    }
    hasher.update(point.x().expect("未压缩点"));
    hasher.update(point.y().expect("未压缩点"));
    Ok(hasher.finalize().into())
}

/// e = SM3(ZA ‖ M)
pub fn digest(za: &[u8; 32], message: &[u8]) -> [u8; 32] {
    Sm3::new()
        .chain_update(za)
        .chain_update(message)
        .finalize()
        .into()
}

/// 实际参与签名的字节，便于跨系统对照
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Details {
    /// 用户标识字节（十六进制）
    pub user_id: String,
    /// ENTLA：用户标识的比特长度
    pub entla: usize,
    pub za: String,
    pub e: String,
    pub message_bytes: usize,
    /// 消息字节的十六进制预览（最多 256 字节）
    pub message_preview: String,
    pub public_key: String,
}

const PREVIEW_BYTES: usize = 256;

fn details(
    user_id: &[u8],
    message: &[u8],
    za: &[u8; 32],
    e: &[u8; 32],
    public: &PublicKey,
) -> Details {
    Details {
        user_id: hex(user_id),
        entla: user_id.len() * 8,
        za: hex(za),
        e: hex(e),
        message_bytes: message.len(),
        message_preview: hex(&message[..message.len().min(PREVIEW_BYTES)]),
        public_key: public_hex(public),
    }
}

// ---------- 签名表示 ----------

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SignatureFormat {
    Der,
    Raw,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SignatureInput {
    Auto,
    Der,
    Raw,
}

fn sig_error(code: &str) -> PluginError {
    PluginError::new(code)
}

/// r、s 必须在 [1, n-1]
fn check_range(value: &[u8; 32], part: &str) -> PluginResult<()> {
    if value.iter().all(|b| *b == 0) || value.as_slice() >= order().as_slice() {
        return Err(sig_error("crypto.sm2_signature_range").with("part", part));
    }
    Ok(())
}

/// 严格的 DER INTEGER：最短编码、非负、不超过 32 字节
fn der_integer<'a>(bytes: &'a [u8], part: &str) -> PluginResult<([u8; 32], &'a [u8])> {
    let malformed = || sig_error("crypto.sm2_signature_der").with("part", part);
    let (&tag, rest) = bytes.split_first().ok_or_else(malformed)?;
    let (&len, rest) = rest.split_first().ok_or_else(malformed)?;
    let len = usize::from(len);
    if tag != 0x02 || len == 0 || len > 0x7f || rest.len() < len {
        return Err(malformed());
    }
    let (content, rest) = rest.split_at(len);
    if content[0] & 0x80 != 0 {
        return Err(sig_error("crypto.sm2_signature_range").with("part", part));
    }
    if len > 1 && content[0] == 0 && content[1] & 0x80 == 0 {
        return Err(sig_error("crypto.sm2_signature_not_minimal").with("part", part));
    }
    let value = content.strip_prefix(&[0]).unwrap_or(content);
    if value.len() > 32 {
        return Err(sig_error("crypto.sm2_signature_range").with("part", part));
    }
    let mut out = [0u8; 32];
    out[32 - value.len()..].copy_from_slice(value);
    check_range(&out, part)?;
    Ok((out, rest))
}

fn parse_der(bytes: &[u8]) -> PluginResult<([u8; 32], [u8; 32])> {
    let malformed = || sig_error("crypto.sm2_signature_der").with("part", "sequence");
    let (&tag, rest) = bytes.split_first().ok_or_else(malformed)?;
    let (&len, body) = rest.split_first().ok_or_else(malformed)?;
    // r、s 最多 33 字节，SEQUENCE 内容不超过 70 字节，只可能是短格式长度
    if tag != 0x30 || len > 0x7f {
        return Err(malformed());
    }
    let len = usize::from(len);
    if body.len() < len {
        return Err(sig_error("crypto.sm2_signature_truncated"));
    }
    if body.len() > len {
        return Err(sig_error("crypto.sm2_signature_trailing").with("bytes", body.len() - len));
    }
    let (r, rest) = der_integer(body, "r")?;
    let (s, rest) = der_integer(rest, "s")?;
    if !rest.is_empty() {
        return Err(sig_error("crypto.sm2_signature_trailing").with("bytes", rest.len()));
    }
    Ok((r, s))
}

fn parse_raw(bytes: &[u8]) -> PluginResult<([u8; 32], [u8; 32])> {
    if bytes.len() != 64 {
        return Err(sig_error("crypto.sm2_signature_length").with("bytes", bytes.len()));
    }
    let r: [u8; 32] = bytes[..32].try_into().expect("32 字节");
    let s: [u8; 32] = bytes[32..].try_into().expect("32 字节");
    check_range(&r, "r")?;
    check_range(&s, "s")?;
    Ok((r, s))
}

/// 解析签名字节；auto 时以 0x30 开头且能严格解析为 DER 的按 DER，否则按 64 字节 r ‖ s
pub fn parse_signature(
    bytes: &[u8],
    format: SignatureInput,
) -> PluginResult<(SignatureFormat, [u8; 32], [u8; 32])> {
    match format {
        SignatureInput::Der => parse_der(bytes).map(|(r, s)| (SignatureFormat::Der, r, s)),
        SignatureInput::Raw => parse_raw(bytes).map(|(r, s)| (SignatureFormat::Raw, r, s)),
        SignatureInput::Auto => {
            if bytes.first() == Some(&0x30)
                && let Ok((r, s)) = parse_der(bytes)
            {
                return Ok((SignatureFormat::Der, r, s));
            }
            if bytes.len() == 64 {
                return parse_raw(bytes).map(|(r, s)| (SignatureFormat::Raw, r, s));
            }
            if bytes.first() == Some(&0x30) {
                // 看起来是 DER：报告 DER 的具体问题
                return parse_der(bytes).map(|(r, s)| (SignatureFormat::Der, r, s));
            }
            Err(sig_error("crypto.sm2_signature_length").with("bytes", bytes.len()))
        }
    }
}

fn der_int(value: &[u8; 32]) -> Vec<u8> {
    let start = value.iter().position(|b| *b != 0).unwrap_or(31);
    let mut content = value[start..].to_vec();
    if content[0] & 0x80 != 0 {
        content.insert(0, 0);
    }
    let mut out = vec![0x02, content.len() as u8];
    out.extend(content);
    out
}

pub fn serialize_signature(r: &[u8; 32], s: &[u8; 32], format: SignatureFormat) -> Vec<u8> {
    match format {
        SignatureFormat::Raw => [r.as_slice(), s.as_slice()].concat(),
        SignatureFormat::Der => {
            let body = [der_int(r), der_int(s)].concat();
            let mut out = vec![0x30, body.len() as u8];
            out.extend(body);
            out
        }
    }
}

// ---------- 签名与验签 ----------

fn failed(err: impl std::fmt::Display) -> PluginError {
    PluginError::new("crypto.sm2_failed").with("detail", err.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub key: String,
    pub user_id: String,
    pub user_id_encoding: Encoding,
    pub message: String,
    pub message_encoding: Encoding,
}

struct Prepared {
    key: Key,
    user_id: Vec<u8>,
    message: Vec<u8>,
    za: [u8; 32],
    e: [u8; 32],
}

fn prepare(input: &Input) -> PluginResult<Prepared> {
    let key = parse_key(&input.key)?;
    let user_id = decode(&input.user_id, input.user_id_encoding, "userId")?;
    let message = decode(&input.message, input.message_encoding, "message")?;
    let za = za(&user_id, &key.public)?;
    let e = digest(&za, &message);
    Ok(Prepared {
        key,
        user_id,
        message,
        za,
        e,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignArgs {
    #[serde(flatten)]
    pub input: Input,
    pub format: SignatureFormat,
    pub encoding: Encoding,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignOutput {
    pub signature: String,
    pub format: SignatureFormat,
    pub r: String,
    pub s: String,
    pub details: Details,
}

pub fn sign(args: SignArgs) -> PluginResult<SignOutput> {
    let p = prepare(&args.input)?;
    let secret = p
        .key
        .secret
        .as_ref()
        .ok_or_else(|| PluginError::new("crypto.sm2_private_required"))?;
    // distid 只用于 crate 自己的 hash_msg；这里直接传入按字节算好的 e
    let signer = SigningKey::new(DEFAULT_USER_ID, secret).map_err(failed)?;
    let signature: Signature = signer.sign_prehash(&p.e).map_err(failed)?;
    let r: [u8; 32] = signature.r().to_bytes().into();
    let s: [u8; 32] = signature.s().to_bytes().into();
    Ok(SignOutput {
        signature: encode(&serialize_signature(&r, &s, args.format), args.encoding)?,
        format: args.format,
        r: hex(&r),
        s: hex(&s),
        details: details(&p.user_id, &p.message, &p.za, &p.e, &p.key.public),
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyArgs {
    #[serde(flatten)]
    pub input: Input,
    pub signature: String,
    pub format: SignatureInput,
    pub encoding: Encoding,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyOutput {
    pub valid: bool,
    pub format: SignatureFormat,
    pub r: String,
    pub s: String,
    pub details: Details,
}

pub fn verify(args: VerifyArgs) -> PluginResult<VerifyOutput> {
    let p = prepare(&args.input)?;
    let bytes = decode(&args.signature, args.encoding, "signature")?;
    let (format, r, s) = parse_signature(&bytes, args.format)?;
    let signature = Signature::from_slice(&[r, s].concat())
        .map_err(|_| sig_error("crypto.sm2_signature_range").with("part", "r"))?;
    let verifier = VerifyingKey::new(DEFAULT_USER_ID, p.key.public).map_err(failed)?;
    Ok(VerifyOutput {
        valid: verifier.verify_prehash(&p.e, &signature).is_ok(),
        format,
        r: hex(&r),
        s: hex(&s),
        details: details(&p.user_id, &p.message, &p.za, &p.e, &p.key.public),
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertArgs {
    pub signature: String,
    pub from: SignatureInput,
    pub encoding: Encoding,
    pub to: SignatureFormat,
    pub output_encoding: Encoding,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertOutput {
    pub signature: String,
    /// 识别出的输入格式
    pub from: SignatureFormat,
    pub r: String,
    pub s: String,
}

pub fn convert(args: ConvertArgs) -> PluginResult<ConvertOutput> {
    let bytes = decode(&args.signature, args.encoding, "signature")?;
    let (from, r, s) = parse_signature(&bytes, args.from)?;
    Ok(ConvertOutput {
        signature: encode(&serialize_signature(&r, &s, args.to), args.output_encoding)?,
        from,
        r: hex(&r),
        s: hex(&s),
    })
}

#[cfg(test)]
#[path = "sm2_test.rs"]
mod tests;
