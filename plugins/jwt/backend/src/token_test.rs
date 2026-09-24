use super::*;
use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};

/// jwt.io 的 HS256 示例
const SAMPLE: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
const SAMPLE_SECRET: &str = "your-256-bit-secret";

fn decode_token(token: &str) -> PluginResult<Decoded> {
    decode(DecodeArgs {
        token: token.into(),
    })
}

fn verify_with(token: &str, key: &str, key_encoding: SecretEncoding) -> Verified {
    verify(VerifyArgs {
        token: token.into(),
        key: key.into(),
        key_encoding,
    })
    .unwrap()
}

fn sign_with(algorithm: &str, key: &str) -> PluginResult<String> {
    sign(SignArgs {
        algorithm: algorithm.into(),
        header: r#"{"kid": "key-1"}"#.into(),
        payload: r#"{"sub": "toolforge", "exp": 4102444800}"#.into(),
        key: key.into(),
        key_encoding: SecretEncoding::Text,
    })
    .map(|s| s.token)
}

/// 测试运行时生成密钥对，避免在仓库中保存私钥
fn rsa_pair() -> (String, String) {
    let key = rsa::RsaPrivateKey::new(&mut rand::thread_rng(), 1024).unwrap();
    let private = key.to_pkcs8_pem(LineEnding::LF).unwrap().to_string();
    let public = rsa::RsaPublicKey::from(&key)
        .to_public_key_pem(LineEnding::LF)
        .unwrap();
    (private, public)
}

fn ec_pair() -> (String, String) {
    use p256::pkcs8::{EncodePrivateKey, EncodePublicKey};
    let key = p256::SecretKey::random(&mut rand::thread_rng());
    let private = key.to_pkcs8_pem(LineEnding::LF).unwrap().to_string();
    let public = key.public_key().to_public_key_pem(LineEnding::LF).unwrap();
    (private, public)
}

fn ed_pair() -> (String, String) {
    use ed25519_dalek::pkcs8::{EncodePrivateKey, EncodePublicKey};
    let key = ed25519_dalek::SigningKey::generate(&mut rand::thread_rng());
    let private = key.to_pkcs8_pem(LineEnding::LF).unwrap().to_string();
    let public = key
        .verifying_key()
        .to_public_key_pem(LineEnding::LF)
        .unwrap();
    (private, public)
}

#[test]
fn decodes_the_jwt_io_sample() {
    let out = decode_token(SAMPLE).unwrap();
    assert_eq!(out.algorithm.as_deref(), Some("HS256"));
    assert!(out.payload.contains("\"name\": \"John Doe\""));
    let iat = out.status.times.iter().find(|t| t.name == "iat").unwrap();
    assert_eq!(iat.iso.as_deref(), Some("2018-01-18T01:30:22Z"));
    assert_eq!(out.status.expired, None);
    assert_eq!(out.segments.header, (0, 36));
    assert_eq!(out.segments.payload.0, 37);
}

#[test]
fn strips_bearer_prefix_and_adjusts_segments() {
    let out = decode_token(&format!("  Bearer {SAMPLE}")).unwrap();
    assert_eq!(out.segments.header, (9, 45));
}

#[test]
fn verifies_hmac_with_text_and_base64_secrets() {
    assert!(verify_with(SAMPLE, SAMPLE_SECRET, SecretEncoding::Text).valid);
    let wrong = verify_with(SAMPLE, "nope", SecretEncoding::Text);
    assert!(!wrong.valid);
    assert_eq!(wrong.reason.as_deref(), Some("jwt.bad_signature"));
    let b64 = data_encoding::BASE64.encode(SAMPLE_SECRET.as_bytes());
    assert!(verify_with(SAMPLE, &b64, SecretEncoding::Base64).valid);
}

#[test]
fn signs_and_verifies_every_key_family() {
    let (rsa_private, rsa_public) = rsa_pair();
    let (ec_private, ec_public) = ec_pair();
    let (ed_private, ed_public) = ed_pair();
    for (alg, private, public) in [
        ("HS384", "secret".to_owned(), "secret".to_owned()),
        ("RS256", rsa_private.clone(), rsa_public.clone()),
        ("PS256", rsa_private, rsa_public),
        ("ES256", ec_private, ec_public),
        ("EdDSA", ed_private, ed_public),
    ] {
        let token = sign_with(alg, &private).unwrap();
        let decoded = decode_token(&token).unwrap();
        assert_eq!(decoded.algorithm.as_deref(), Some(alg), "{alg}");
        assert!(decoded.header.contains("key-1"), "{alg}");
        assert_eq!(decoded.status.expired, Some(false));
        assert!(
            verify_with(&token, &public, SecretEncoding::Text).valid,
            "{alg}"
        );
    }
}

#[test]
fn rejects_signatures_from_other_keys() {
    let (private, _) = rsa_pair();
    let (_, other_public) = rsa_pair();
    let token = sign_with("RS256", &private).unwrap();
    assert!(!verify_with(&token, &other_public, SecretEncoding::Text).valid);
}

#[test]
fn refuses_alg_none_and_bad_keys() {
    let none = "eyJhbGciOiJub25lIn0.eyJzdWIiOiJ4In0.";
    let err = verify(VerifyArgs {
        token: none.into(),
        key: "k".into(),
        key_encoding: SecretEncoding::Text,
    })
    .unwrap_err();
    assert_eq!(err.code, "jwt.none_alg");
    assert_eq!(sign_with("none", "k").unwrap_err().code, "jwt.none_alg");
    assert_eq!(
        sign_with("RS256", "not a pem").unwrap_err().code,
        "jwt.invalid_key"
    );
    assert_eq!(
        sign_with("HS256", "   ").unwrap_err().code,
        "jwt.key_required"
    );
}

#[test]
fn reports_malformed_tokens() {
    assert_eq!(decode_token("").unwrap_err().code, "jwt.empty");
    assert_eq!(decode_token("a.b").unwrap_err().code, "jwt.malformed");
    assert_eq!(
        decode_token("!!!.e30.x").unwrap_err().code,
        "jwt.invalid_base64"
    );
    let not_json = format!("{}.e30.x", data_encoding::BASE64URL_NOPAD.encode(b"nope"));
    assert_eq!(
        decode_token(&not_json).unwrap_err().code,
        "jwt.invalid_json"
    );
}

#[test]
fn payload_must_be_an_object() {
    let err = sign(SignArgs {
        algorithm: "HS256".into(),
        header: String::new(),
        payload: "[1, 2]".into(),
        key: "k".into(),
        key_encoding: SecretEncoding::Text,
    })
    .unwrap_err();
    assert_eq!(err.code, "jwt.payload_not_object");
}
