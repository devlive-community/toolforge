use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey};
use serde::Deserialize;
use tf_plugin_api::{PluginError, PluginResult};

/// HMAC 密钥的书写方式
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SecretEncoding {
    #[default]
    Text,
    Base64,
}

fn invalid_key(detail: impl ToString) -> PluginError {
    PluginError::new("jwt.invalid_key").with("detail", detail.to_string())
}

pub enum Family {
    Hmac,
    Rsa,
    Ec,
    Ed,
}

pub fn family(alg: Algorithm) -> Family {
    match alg {
        Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512 => Family::Hmac,
        Algorithm::ES256 | Algorithm::ES384 => Family::Ec,
        Algorithm::EdDSA => Family::Ed,
        _ => Family::Rsa,
    }
}

pub fn decoding(alg: Algorithm, key: &str, encoding: SecretEncoding) -> PluginResult<DecodingKey> {
    if key.trim().is_empty() {
        return Err(PluginError::new("jwt.key_required"));
    }
    match family(alg) {
        Family::Hmac => match encoding {
            SecretEncoding::Text => Ok(DecodingKey::from_secret(key.as_bytes())),
            SecretEncoding::Base64 => {
                DecodingKey::from_base64_secret(key.trim()).map_err(invalid_key)
            }
        },
        Family::Rsa => DecodingKey::from_rsa_pem(key.trim().as_bytes()).map_err(invalid_key),
        Family::Ec => DecodingKey::from_ec_pem(key.trim().as_bytes()).map_err(invalid_key),
        Family::Ed => DecodingKey::from_ed_pem(key.trim().as_bytes()).map_err(invalid_key),
    }
}

pub fn encoding(alg: Algorithm, key: &str, encoding: SecretEncoding) -> PluginResult<EncodingKey> {
    if key.trim().is_empty() {
        return Err(PluginError::new("jwt.key_required"));
    }
    match family(alg) {
        Family::Hmac => match encoding {
            SecretEncoding::Text => Ok(EncodingKey::from_secret(key.as_bytes())),
            SecretEncoding::Base64 => {
                EncodingKey::from_base64_secret(key.trim()).map_err(invalid_key)
            }
        },
        Family::Rsa => EncodingKey::from_rsa_pem(key.trim().as_bytes()).map_err(invalid_key),
        Family::Ec => EncodingKey::from_ec_pem(key.trim().as_bytes()).map_err(invalid_key),
        Family::Ed => EncodingKey::from_ed_pem(key.trim().as_bytes()).map_err(invalid_key),
    }
}
