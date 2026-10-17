//! RSA：生成密钥对，OAEP / PKCS#1 v1.5 加解密，PKCS#1 v1.5 / PSS 签名与验签。
//! 支持 PKCS#8（BEGIN PRIVATE KEY / PUBLIC KEY）与 PKCS#1（BEGIN RSA …）两种 PEM。

use rsa::pkcs1::{
    DecodeRsaPrivateKey, DecodeRsaPublicKey, EncodeRsaPrivateKey, EncodeRsaPublicKey,
};
use rsa::pkcs8::{
    DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey, LineEnding,
};
use rsa::traits::PublicKeyParts;
use rsa::{Oaep, Pkcs1v15Encrypt, Pkcs1v15Sign, Pss, RsaPrivateKey, RsaPublicKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha384, Sha512};
use tf_plugin_api::{PluginError, PluginResult};

use crate::codec::{Encoding, decode, encode};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum KeyFormat {
    Pkcs8,
    Pkcs1,
}

#[derive(Debug, Deserialize)]
pub struct GenerateArgs {
    pub bits: usize,
    pub format: KeyFormat,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyPair {
    pub private_key: String,
    pub public_key: String,
    pub bits: usize,
}

fn failed(err: impl std::fmt::Display) -> PluginError {
    PluginError::new("crypto.rsa_failed").with("detail", err.to_string())
}

pub fn generate(args: GenerateArgs) -> PluginResult<KeyPair> {
    if ![1024, 2048, 3072, 4096].contains(&args.bits) {
        return Err(PluginError::new("crypto.invalid_key_bits").with("bits", args.bits));
    }
    let private = RsaPrivateKey::new(&mut rand::rngs::OsRng, args.bits).map_err(failed)?;
    let public = private.to_public_key();
    let (private_key, public_key) = match args.format {
        KeyFormat::Pkcs8 => (
            private
                .to_pkcs8_pem(LineEnding::LF)
                .map_err(failed)?
                .to_string(),
            public.to_public_key_pem(LineEnding::LF).map_err(failed)?,
        ),
        KeyFormat::Pkcs1 => (
            private
                .to_pkcs1_pem(LineEnding::LF)
                .map_err(failed)?
                .to_string(),
            public.to_pkcs1_pem(LineEnding::LF).map_err(failed)?,
        ),
    };
    Ok(KeyPair {
        private_key,
        public_key,
        bits: args.bits,
    })
}

fn invalid_key() -> PluginError {
    PluginError::new("crypto.invalid_key_pem")
}

pub fn private_key(pem: &str) -> PluginResult<RsaPrivateKey> {
    let pem = pem.trim();
    if pem.contains("ENCRYPTED PRIVATE KEY") || pem.contains("Proc-Type: 4,ENCRYPTED") {
        return Err(PluginError::new("crypto.encrypted_key"));
    }
    RsaPrivateKey::from_pkcs8_pem(pem)
        .or_else(|_| RsaPrivateKey::from_pkcs1_pem(pem))
        .map_err(|_| invalid_key())
}

/// 公钥；传入私钥时取其公钥部分
pub fn public_key(pem: &str) -> PluginResult<RsaPublicKey> {
    let pem = pem.trim();
    RsaPublicKey::from_public_key_pem(pem)
        .or_else(|_| RsaPublicKey::from_pkcs1_pem(pem))
        .or_else(|_| private_key(pem).map(|k| k.to_public_key()))
        .map_err(|_| invalid_key())
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KeyInfo {
    /// private / public
    pub kind: &'static str,
    pub bits: usize,
    /// 公钥的 PEM（从私钥推出时也会返回，便于分发）
    pub public_key: String,
}

pub fn inspect(pem: &str) -> PluginResult<KeyInfo> {
    let (kind, public) = match private_key(pem) {
        Ok(private) => ("private", private.to_public_key()),
        Err(err) if err.code == "crypto.encrypted_key" => return Err(err),
        Err(_) => ("public", public_key(pem)?),
    };
    Ok(KeyInfo {
        kind,
        bits: public.n().bits(),
        public_key: public.to_public_key_pem(LineEnding::LF).map_err(failed)?,
    })
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Padding {
    OaepSha256,
    OaepSha1,
    Pkcs1v15,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CipherArgs {
    pub key: String,
    pub input: String,
    pub input_encoding: Encoding,
    pub output_encoding: Encoding,
    pub padding: Padding,
}

/// 单次加密的最大明文长度
pub fn max_plaintext(bits: usize, padding: Padding) -> usize {
    let size = bits / 8;
    match padding {
        Padding::OaepSha256 => size.saturating_sub(2 * 32 + 2),
        Padding::OaepSha1 => size.saturating_sub(2 * 20 + 2),
        Padding::Pkcs1v15 => size.saturating_sub(11),
    }
}

pub fn encrypt(args: CipherArgs) -> PluginResult<String> {
    let key = public_key(&args.key)?;
    let data = decode(&args.input, args.input_encoding, "input")?;
    let max = max_plaintext(key.n().bits(), args.padding);
    if data.len() > max {
        return Err(PluginError::new("crypto.message_too_long")
            .with("max", max)
            .with("actual", data.len()));
    }
    let mut rng = rand::rngs::OsRng;
    let cipher = match args.padding {
        Padding::OaepSha256 => key.encrypt(&mut rng, Oaep::new::<Sha256>(), &data),
        Padding::OaepSha1 => key.encrypt(&mut rng, Oaep::new::<sha1::Sha1>(), &data),
        Padding::Pkcs1v15 => key.encrypt(&mut rng, Pkcs1v15Encrypt, &data),
    }
    .map_err(failed)?;
    encode(&cipher, args.output_encoding)
}

pub fn decrypt(args: CipherArgs) -> PluginResult<String> {
    let key = private_key(&args.key)?;
    let data = decode(&args.input, args.input_encoding, "input")?;
    let plain = match args.padding {
        Padding::OaepSha256 => key.decrypt(Oaep::new::<Sha256>(), &data),
        Padding::OaepSha1 => key.decrypt(Oaep::new::<sha1::Sha1>(), &data),
        Padding::Pkcs1v15 => key.decrypt(Pkcs1v15Encrypt, &data),
    }
    .map_err(|_| PluginError::new("crypto.decrypt_failed"))?;
    encode(&plain, args.output_encoding)
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Scheme {
    Pkcs1v15,
    Pss,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Hash {
    Sha256,
    Sha384,
    Sha512,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignArgs {
    pub key: String,
    pub message: String,
    pub message_encoding: Encoding,
    pub scheme: Scheme,
    pub hash: Hash,
    /// 签名的编码（签名输出 / 验签输入）
    pub signature_encoding: Encoding,
    /// 验签时的签名
    #[serde(default)]
    pub signature: String,
}

fn digest(hash: Hash, data: &[u8]) -> Vec<u8> {
    match hash {
        Hash::Sha256 => Sha256::digest(data).to_vec(),
        Hash::Sha384 => Sha384::digest(data).to_vec(),
        Hash::Sha512 => Sha512::digest(data).to_vec(),
    }
}

macro_rules! with_hash {
    ($hash:expr, $scheme:expr, |$padding:ident| $body:expr) => {
        match ($scheme, $hash) {
            (Scheme::Pkcs1v15, Hash::Sha256) => {
                let $padding = Pkcs1v15Sign::new::<Sha256>();
                $body
            }
            (Scheme::Pkcs1v15, Hash::Sha384) => {
                let $padding = Pkcs1v15Sign::new::<Sha384>();
                $body
            }
            (Scheme::Pkcs1v15, Hash::Sha512) => {
                let $padding = Pkcs1v15Sign::new::<Sha512>();
                $body
            }
            (Scheme::Pss, Hash::Sha256) => {
                let $padding = Pss::new::<Sha256>();
                $body
            }
            (Scheme::Pss, Hash::Sha384) => {
                let $padding = Pss::new::<Sha384>();
                $body
            }
            (Scheme::Pss, Hash::Sha512) => {
                let $padding = Pss::new::<Sha512>();
                $body
            }
        }
    };
}

pub fn sign(args: SignArgs) -> PluginResult<String> {
    let key = private_key(&args.key)?;
    let hashed = digest(
        args.hash,
        &decode(&args.message, args.message_encoding, "message")?,
    );
    let signature = with_hash!(args.hash, args.scheme, |padding| key.sign_with_rng(
        &mut rand::rngs::OsRng,
        padding,
        &hashed
    ))
    .map_err(failed)?;
    encode(&signature, args.signature_encoding)
}

pub fn verify(args: SignArgs) -> PluginResult<bool> {
    let key = public_key(&args.key)?;
    let hashed = digest(
        args.hash,
        &decode(&args.message, args.message_encoding, "message")?,
    );
    let signature = decode(&args.signature, args.signature_encoding, "signature")?;
    Ok(with_hash!(args.hash, args.scheme, |padding| key
        .verify(padding, &hashed, &signature))
    .is_ok())
}

#[cfg(test)]
#[path = "rsa_test.rs"]
mod tests;
