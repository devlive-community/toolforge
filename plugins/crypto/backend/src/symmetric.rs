//! 对称加密：AES（GCM / CBC / CTR / ECB）、SM4（同样的模式，GCM 即 RFC 8998）与 ChaCha20-Poly1305。
//! 密钥可以直接给出，也可以用 PBKDF2-HMAC-SHA256 从口令派生。

use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyInit, KeyIvInit, StreamCipher};
use aes::{Aes128, Aes192, Aes256};
use aes_gcm::AesGcm;
use aes_gcm::aead::consts::U12;
use aes_gcm::aead::{Aead, Payload};
use cbc::cipher::block_padding::{NoPadding, Pkcs7, ZeroPadding};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sm4::Sm4;
use tf_plugin_api::{PluginError, PluginResult};

use crate::codec::{Encoding, decode, encode, hex};

pub const MAX_INPUT: usize = 10 * 1024 * 1024;
const SALT_LEN: usize = 16;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Algorithm {
    Aes,
    Sm4,
    Chacha20,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Gcm,
    Cbc,
    Ctr,
    Ecb,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Encrypt,
    Decrypt,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum KeyEncoding {
    Hex,
    Base64,
    Utf8,
    /// 口令：用 PBKDF2 派生密钥
    Password,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Padding {
    #[default]
    Pkcs7,
    Zero,
    None,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub algorithm: Algorithm,
    pub mode: Mode,
    pub direction: Direction,
    pub input: String,
    pub input_encoding: Encoding,
    pub output_encoding: Encoding,
    pub key: String,
    pub key_encoding: KeyEncoding,
    /// AES 的密钥位数（口令派生时决定长度）
    #[serde(default = "default_bits")]
    pub key_bits: u16,
    /// 十六进制；加密时为空则随机生成
    #[serde(default)]
    pub iv: String,
    /// 把 IV / nonce 放在密文最前面（解密时从密文开头读取）
    #[serde(default)]
    pub embed_iv: bool,
    /// 口令派生用的盐（十六进制）；加密时为空则随机生成
    #[serde(default)]
    pub salt: String,
    #[serde(default = "default_iterations")]
    pub iterations: u32,
    /// GCM / ChaCha20-Poly1305 的附加认证数据（UTF-8）
    #[serde(default)]
    pub aad: String,
    #[serde(default)]
    pub padding: Padding,
}

fn default_bits() -> u16 {
    256
}

fn default_iterations() -> u32 {
    100_000
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub output: String,
    /// 实际使用的 IV / nonce（十六进制），ECB 为空
    pub iv: Option<String>,
    /// 口令派生使用的盐
    pub salt: Option<String>,
    /// 派生出的密钥（十六进制），仅口令模式返回
    pub derived_key: Option<String>,
    pub bytes: usize,
}

fn key_len(args: &Args) -> PluginResult<usize> {
    match args.algorithm {
        Algorithm::Aes => match args.key_bits {
            128 | 192 | 256 => Ok(args.key_bits as usize / 8),
            other => Err(PluginError::new("crypto.invalid_key_bits").with("bits", other)),
        },
        Algorithm::Sm4 => Ok(16),
        Algorithm::Chacha20 => Ok(32),
    }
}

/// GCM 与 ChaCha20 用 12 字节 nonce，CBC / CTR 用 16 字节 IV，ECB 不需要
fn iv_len(args: &Args) -> usize {
    match (args.algorithm, args.mode) {
        (Algorithm::Chacha20, _) | (_, Mode::Gcm) => 12,
        (_, Mode::Ecb) => 0,
        _ => 16,
    }
}

fn random(len: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; len];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes
}

fn derive(password: &str, salt: &[u8], iterations: u32, len: usize) -> Vec<u8> {
    let mut key = vec![0u8; len];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(password.as_bytes(), salt, iterations.max(1), &mut key);
    key
}

fn decrypt_failed() -> PluginError {
    PluginError::new("crypto.decrypt_failed")
}

macro_rules! block_mode {
    ($cipher:ty, $args:expr, $key:expr, $iv:expr, $data:expr) => {{
        let encrypt = $args.direction == Direction::Encrypt;
        match $args.mode {
            Mode::Cbc => {
                if encrypt {
                    let c = cbc::Encryptor::<$cipher>::new_from_slices($key, $iv)
                        .map_err(|_| decrypt_failed())?;
                    match $args.padding {
                        Padding::Pkcs7 => Ok(c.encrypt_padded_vec_mut::<Pkcs7>($data)),
                        Padding::Zero => Ok(c.encrypt_padded_vec_mut::<ZeroPadding>($data)),
                        Padding::None => {
                            aligned($data).map(|_| c.encrypt_padded_vec_mut::<NoPadding>($data))
                        }
                    }
                } else {
                    let c = cbc::Decryptor::<$cipher>::new_from_slices($key, $iv)
                        .map_err(|_| decrypt_failed())?;
                    match $args.padding {
                        Padding::Pkcs7 => c
                            .decrypt_padded_vec_mut::<Pkcs7>($data)
                            .map_err(|_| decrypt_failed()),
                        Padding::Zero => c
                            .decrypt_padded_vec_mut::<ZeroPadding>($data)
                            .map_err(|_| decrypt_failed()),
                        Padding::None => c
                            .decrypt_padded_vec_mut::<NoPadding>($data)
                            .map_err(|_| decrypt_failed()),
                    }
                }
            }
            Mode::Ecb => {
                if encrypt {
                    let c = ecb::Encryptor::<$cipher>::new_from_slice($key)
                        .map_err(|_| decrypt_failed())?;
                    match $args.padding {
                        Padding::Pkcs7 => Ok(c.encrypt_padded_vec_mut::<Pkcs7>($data)),
                        Padding::Zero => Ok(c.encrypt_padded_vec_mut::<ZeroPadding>($data)),
                        Padding::None => {
                            aligned($data).map(|_| c.encrypt_padded_vec_mut::<NoPadding>($data))
                        }
                    }
                } else {
                    let c = ecb::Decryptor::<$cipher>::new_from_slice($key)
                        .map_err(|_| decrypt_failed())?;
                    match $args.padding {
                        Padding::Pkcs7 => c
                            .decrypt_padded_vec_mut::<Pkcs7>($data)
                            .map_err(|_| decrypt_failed()),
                        Padding::Zero => c
                            .decrypt_padded_vec_mut::<ZeroPadding>($data)
                            .map_err(|_| decrypt_failed()),
                        Padding::None => c
                            .decrypt_padded_vec_mut::<NoPadding>($data)
                            .map_err(|_| decrypt_failed()),
                    }
                }
            }
            Mode::Ctr => {
                let mut c = ctr::Ctr128BE::<$cipher>::new_from_slices($key, $iv)
                    .map_err(|_| decrypt_failed())?;
                let mut buffer = $data.to_vec();
                c.apply_keystream(&mut buffer);
                Ok(buffer)
            }
            Mode::Gcm => {
                let c =
                    AesGcm::<$cipher, U12>::new_from_slice($key).map_err(|_| decrypt_failed())?;
                let payload = Payload {
                    msg: $data,
                    aad: $args.aad.as_bytes(),
                };
                let nonce = aes_gcm::Nonce::from_slice($iv);
                if encrypt {
                    c.encrypt(nonce, payload).map_err(|_| decrypt_failed())
                } else {
                    c.decrypt(nonce, payload).map_err(|_| decrypt_failed())
                }
            }
        }
    }};
}

/// 不填充时明文长度必须是 16 的倍数
fn aligned(data: &[u8]) -> PluginResult<()> {
    if data.len().is_multiple_of(16) {
        Ok(())
    } else {
        Err(PluginError::new("crypto.not_aligned").with("length", data.len()))
    }
}

pub fn run(args: Args) -> PluginResult<Output> {
    if args.input.len() > MAX_INPUT {
        return Err(PluginError::new("crypto.too_large").with("limit", "10 MB"));
    }
    if args.input.is_empty() {
        return Err(PluginError::new("crypto.empty_input"));
    }
    let encrypt = args.direction == Direction::Encrypt;
    let key_len = key_len(&args)?;
    let iv_len = iv_len(&args);

    // 密钥
    let (key, salt) = if args.key_encoding == KeyEncoding::Password {
        if args.key.is_empty() {
            return Err(PluginError::new("crypto.empty_key"));
        }
        let salt = if args.salt.trim().is_empty() {
            if !encrypt {
                return Err(PluginError::new("crypto.salt_required"));
            }
            random(SALT_LEN)
        } else {
            decode(&args.salt, Encoding::Hex, "salt")?
        };
        (
            derive(&args.key, &salt, args.iterations, key_len),
            Some(salt),
        )
    } else {
        let encoding = match args.key_encoding {
            KeyEncoding::Hex => Encoding::Hex,
            KeyEncoding::Base64 => Encoding::Base64,
            _ => Encoding::Utf8,
        };
        (decode(&args.key, encoding, "key")?, None)
    };
    if key.len() != key_len {
        return Err(PluginError::new("crypto.invalid_key_length")
            .with("expected", key_len)
            .with("actual", key.len()));
    }

    let mut data = decode(&args.input, args.input_encoding, "input")?;

    // IV：显式给出、嵌在密文开头，或（加密时）随机生成
    let iv = if iv_len == 0 {
        Vec::new()
    } else if !args.iv.trim().is_empty() {
        decode(&args.iv, Encoding::Hex, "iv")?
    } else if !encrypt && args.embed_iv {
        if data.len() < iv_len {
            return Err(decrypt_failed());
        }
        let rest = data.split_off(iv_len);
        std::mem::replace(&mut data, rest)
    } else if encrypt {
        random(iv_len)
    } else {
        return Err(PluginError::new("crypto.iv_required"));
    };
    if iv.len() != iv_len {
        return Err(PluginError::new("crypto.invalid_iv_length")
            .with("expected", iv_len)
            .with("actual", iv.len()));
    }

    let result = match (args.algorithm, key_len) {
        (Algorithm::Chacha20, _) => {
            use chacha20poly1305::{ChaCha20Poly1305, Nonce};
            let cipher = ChaCha20Poly1305::new_from_slice(&key).map_err(|_| decrypt_failed())?;
            let payload = Payload {
                msg: &data,
                aad: args.aad.as_bytes(),
            };
            if encrypt {
                cipher
                    .encrypt(Nonce::from_slice(&iv), payload)
                    .map_err(|_| decrypt_failed())
            } else {
                cipher
                    .decrypt(Nonce::from_slice(&iv), payload)
                    .map_err(|_| decrypt_failed())
            }
        }
        (Algorithm::Sm4, _) => block_mode!(Sm4, args, &key, &iv, &data),
        (Algorithm::Aes, 16) => block_mode!(Aes128, args, &key, &iv, &data),
        (Algorithm::Aes, 24) => block_mode!(Aes192, args, &key, &iv, &data),
        (Algorithm::Aes, _) => block_mode!(Aes256, args, &key, &iv, &data),
    }?;

    let bytes = if encrypt && args.embed_iv {
        [iv.clone(), result].concat()
    } else {
        result
    };
    let output = encode(&bytes, args.output_encoding)?;
    Ok(Output {
        output,
        iv: (!iv.is_empty()).then(|| hex(&iv)),
        derived_key: salt.as_ref().map(|_| hex(&key)),
        salt: salt.map(|s| hex(&s)),
        bytes: bytes.len(),
    })
}

#[cfg(test)]
#[path = "symmetric_test.rs"]
mod tests;
