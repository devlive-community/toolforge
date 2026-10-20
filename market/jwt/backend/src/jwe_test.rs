use super::*;
use aes_gcm::Aes256Gcm;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use rsa::Oaep;
use rsa::pkcs8::EncodePrivateKey;
use sha2::Sha256;

/// 独立加密方向构造合法 JWE（测试向量生成器）
fn encrypt_dir(cek: &[u8], iv: &[u8; 12], plaintext: &[u8], header_json: &str) -> String {
    let header = b64url_encode(header_json.as_bytes());
    let cipher = Aes256Gcm::new_from_slice(cek).unwrap();
    let sealed = cipher
        .encrypt(
            iv.into(),
            Payload {
                msg: plaintext,
                // RFC 7516: AAD = ASCII(BASE64URL(UTF8(protected header)))
                aad: header.as_bytes(),
            },
        )
        .unwrap();
    let (ciphertext, tag) = sealed.split_at(sealed.len() - 16);
    format!(
        "{header}..{iv_b64}.{ct_b64}.{tag_b64}",
        iv_b64 = b64url_encode(iv),
        ct_b64 = b64url_encode(ciphertext),
        tag_b64 = b64url_encode(tag)
    )
}

fn b64url_encode(data: &[u8]) -> String {
    BASE64URL_NOPAD.encode(data)
}

fn b64url_json(value: &serde_json::Value) -> String {
    b64url_encode(value.to_string().as_bytes())
}

const KEY: [u8; 32] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
    0x0f, //
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
];

fn dir_args(token: &str) -> DecryptArgs {
    DecryptArgs {
        token: token.to_owned(),
        key_text: Some(hex(&KEY)),
        key_encoding: Some("hex".to_owned()),
        private_pem: None,
    }
}

fn hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn dir_roundtrip_utf8_and_json() {
    let iv = [7u8; 12];
    let token = encrypt_dir(&KEY, &iv, b"hello jwe", r#"{"alg":"dir","enc":"A256GCM"}"#);
    let out = decrypt(dir_args(&token)).unwrap();
    assert_eq!(out.kind, "utf8");
    assert_eq!(out.plaintext, "hello jwe");
    assert_eq!(out.alg, "dir");
    assert_eq!(out.enc, "A256GCM");
    assert_eq!(out.encrypted_key_len, 0);

    let token = encrypt_dir(
        &KEY,
        &iv,
        br#"{"sub":"a1"}"#,
        r#"{"alg":"dir","enc":"A256GCM"}"#,
    );
    let out = decrypt(dir_args(&token)).unwrap();
    assert_eq!(out.kind, "json");
    assert_eq!(out.header["alg"], "dir");
}

#[test]
fn dir_empty_plaintext_allowed() {
    let iv = [5u8; 12];
    let token = encrypt_dir(&KEY, &iv, b"", r#"{"alg":"dir","enc":"A256GCM"}"#);
    let out = decrypt(dir_args(&token)).unwrap();
    assert_eq!(out.plaintext, "");
    assert_eq!(out.kind, "utf8");
}

#[test]
fn dir_accepts_base64url_key() {
    let iv = [9u8; 12];
    let token = encrypt_dir(&KEY, &iv, b"x", r#"{"alg":"dir","enc":"A256GCM"}"#);
    let args = DecryptArgs {
        token,
        key_text: Some(b64url_encode(&KEY)),
        key_encoding: Some("base64url".to_owned()),
        private_pem: None,
    };
    assert_eq!(decrypt(args).unwrap().plaintext, "x");
}

#[test]
fn rsa_oaep_256_roundtrip() {
    let mut rng = rand::thread_rng();
    let private_key = rsa::RsaPrivateKey::new(&mut rng, 2048).unwrap();
    let pem = private_key
        .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
        .unwrap()
        .to_string();
    // 独立方向：RSA-OAEP(SHA-256) 加密 CEK + A256GCM 加密载荷
    let cek = [0x42u8; 32];
    let encrypted_key = private_key
        .to_public_key()
        .encrypt(&mut rng, Oaep::new::<Sha256>(), &cek)
        .unwrap();
    let iv = [3u8; 12];
    let header = r#"{"alg":"RSA-OAEP-256","enc":"A256GCM"}"#;
    let header_b64 = b64url_json(&serde_json::from_str::<serde_json::Value>(header).unwrap());
    let cipher = Aes256Gcm::new_from_slice(&cek).unwrap();
    let sealed = cipher
        .encrypt(
            (&iv).into(),
            Payload {
                msg: b"rsa payload",
                aad: header_b64.as_bytes(),
            },
        )
        .unwrap();
    let (ciphertext, tag) = sealed.split_at(sealed.len() - 16);
    let token = format!(
        "{header_b64}.{ek}.{iv}.{ct}.{tag}",
        ek = b64url_encode(&encrypted_key),
        iv = b64url_encode(&iv),
        ct = b64url_encode(ciphertext),
        tag = b64url_encode(tag)
    );
    let out = decrypt(DecryptArgs {
        token,
        key_text: None,
        key_encoding: None,
        private_pem: Some(pem),
    })
    .unwrap();
    assert_eq!(out.plaintext, "rsa payload");
    assert_eq!(out.kind, "utf8");
}

#[test]
fn wrong_key_and_tampering_fail() {
    let iv = [1u8; 12];
    let token = encrypt_dir(&KEY, &iv, b"secret", r#"{"alg":"dir","enc":"A256GCM"}"#);
    // 错误密钥：统一认证失败
    let mut wrong_key = KEY;
    wrong_key[0] ^= 0xff;
    let args = DecryptArgs {
        token: token.clone(),
        key_text: Some(hex(&wrong_key)),
        key_encoding: Some("hex".to_owned()),
        private_pem: None,
    };
    assert_eq!(decrypt(args).unwrap_err().code, "jwe.decrypt_failed");
    // 密文篡改
    let segments: Vec<String> = token.split('.').map(str::to_owned).collect();
    let mut tampered_ct = BASE64URL_NOPAD.decode(segments[3].as_bytes()).unwrap();
    tampered_ct[0] ^= 1;
    let tampered = format!(
        "{}.{}.{}.{}.{}",
        segments[0],
        segments[1],
        segments[2],
        b64url_encode(&tampered_ct),
        segments[4]
    );
    assert_eq!(
        decrypt(dir_args(&tampered)).unwrap_err().code,
        "jwe.decrypt_failed"
    );
    // protected header 篡改（AAD 变化）：改写 JSON 后重新编码也应失败
    let tampered_header = b64url_json(&serde_json::json!({"alg":"dir","enc":"A256GCM","extra":1}));
    let tampered = format!(
        "{}.{}.{}.{}.{}",
        tampered_header, segments[1], segments[2], segments[3], segments[4]
    );
    assert_eq!(
        decrypt(dir_args(&tampered)).unwrap_err().code,
        "jwe.decrypt_failed"
    );
}

#[test]
fn unsupported_headers_and_combos_rejected() {
    let iv = [2u8; 12];
    let token = encrypt_dir(&KEY, &iv, b"x", r#"{"alg":"dir","enc":"A256GCM"}"#);
    // crit 扩展（五段构造，crit 在后续段解析前检查）
    let crit = b64url_json(&serde_json::json!({"alg":"dir","enc":"A256GCM","crit":["exp"]}));
    let crit_token = crit + ".a.b.c.d";
    assert_eq!(
        decrypt(dir_args(&crit_token)).unwrap_err().code,
        "jwe.crit_unsupported"
    );
    // zip
    let zip = b64url_json(&serde_json::json!({"alg":"dir","enc":"A256GCM","zip":"DEF"}));
    let zip_token = zip + ".a.b.c.d";
    assert_eq!(
        decrypt(dir_args(&zip_token)).unwrap_err().code,
        "jwe.zip_unsupported"
    );
    // 不允许的组合
    let combo = b64url_json(&serde_json::json!({"alg":"dir","enc":"A128GCM"}));
    let combo_token = combo + ".a.b.c.d";
    assert_eq!(
        decrypt(dir_args(&combo_token)).unwrap_err().code,
        "jwe.unsupported_combo"
    );
    let combo = b64url_json(&serde_json::json!({"alg":"RSA1_5","enc":"A256GCM"}));
    let combo_token = combo + ".a.b.c.d";
    assert_eq!(
        decrypt(dir_args(&combo_token)).unwrap_err().code,
        "jwe.unsupported_combo"
    );
    // dir 携带非空 encrypted_key（其余段保持合法，确保先走到 CEK 检查）
    let segments: Vec<&str> = token.split('.').collect();
    let ek_token = format!(
        "{}.abc.{}.{}.{}",
        segments[0], segments[2], segments[3], segments[4]
    );
    assert_eq!(
        decrypt(dir_args(&ek_token)).unwrap_err().code,
        "jwe.encrypted_key_expected_empty"
    );
    // 非法长度：IV 11 字节
    let token = encrypt_dir(&KEY, &iv, b"x", r#"{"alg":"dir","enc":"A256GCM"}"#);
    let segments: Vec<&str> = token.split('.').collect();
    let short_iv = b64url_encode(&iv[..11]);
    let bad_iv = format!(
        "{}.{}.{}.{}.{}",
        segments[0], segments[1], short_iv, segments[3], segments[4]
    );
    assert_eq!(
        decrypt(dir_args(&bad_iv)).unwrap_err().code,
        "jwe.bad_iv_len"
    );
    // 段数错误
    assert_eq!(
        decrypt(dir_args("a.b.c")).unwrap_err().code,
        "jwe.bad_parts"
    );
    // 缺 enc
    let no_enc = b64url_json(&serde_json::json!({"alg":"dir"}));
    let no_enc_token = no_enc + ".a.b.c.d";
    assert_eq!(
        decrypt(dir_args(&no_enc_token)).unwrap_err().code,
        "jwe.missing_enc"
    );
    // 非严格 base64url（标准 base64 的 + / 字符）
    assert_eq!(
        decrypt(dir_args("a+b/c.d.e.f.g")).unwrap_err().code,
        "jwe.bad_base64url"
    );
    // 长度上限
    let long_token = "a".repeat(MAX_TOKEN + 1);
    assert_eq!(
        decrypt(dir_args(&long_token)).unwrap_err().code,
        "jwe.too_large"
    );
}

#[test]
fn parse_jwe_without_key_shows_header_and_parts() {
    let iv = [7u8; 12];
    let token = encrypt_dir(&KEY, &iv, b"hello jwe", r#"{"alg":"dir","enc":"A256GCM"}"#);
    let out = parse(ParseArgs { token }).unwrap();
    assert_eq!(out.header["alg"], "dir");
    assert_eq!(out.header["enc"], "A256GCM");
    assert_eq!(out.alg, "dir");
    assert_eq!(out.encrypted_key_len, 0);
    // 五段长度：header、空 encrypted key、IV 12 字节、密文、tag 16 字节
    let expected_header_len = b64url_encode(r#"{"alg":"dir","enc":"A256GCM"}"#.as_bytes()).len();
    assert_eq!(out.parts[0], expected_header_len);
    assert_eq!(out.parts[1], 0);
    assert_eq!(out.parts[2], b64url_encode(&iv).len());
    assert_eq!(out.parts[4], 22);
    // 解析不校验密钥段内容合法性之外的解密参数
    assert_eq!(
        parse(ParseArgs {
            token: "a.b.c".to_owned()
        })
        .unwrap_err()
        .code,
        "jwe.bad_parts"
    );
}

#[test]
fn binary_plaintext_reported() {
    let iv = [8u8; 12];
    let token = encrypt_dir(
        &KEY,
        &iv,
        &[0, 159, 146, 150],
        r#"{"alg":"dir","enc":"A256GCM"}"#,
    );
    let out = decrypt(dir_args(&token)).unwrap();
    assert_eq!(out.kind, "binary");
    assert_eq!(out.plaintext_base64, "AJ-Slg");
    assert_eq!(out.plaintext, "");
}
