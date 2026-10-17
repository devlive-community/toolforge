use std::sync::OnceLock;

use super::*;

/// 测试共用的 2048 位密钥（生成较慢，只生成一次）
fn pair() -> &'static KeyPair {
    static PAIR: OnceLock<KeyPair> = OnceLock::new();
    PAIR.get_or_init(|| {
        generate(GenerateArgs {
            bits: 2048,
            format: KeyFormat::Pkcs8,
        })
        .unwrap()
    })
}

fn cipher(
    key: &str,
    input: &str,
    padding: Padding,
    input_encoding: Encoding,
    output_encoding: Encoding,
) -> CipherArgs {
    CipherArgs {
        key: key.into(),
        input: input.into(),
        input_encoding,
        output_encoding,
        padding,
    }
}

#[test]
fn generates_and_inspects_keys() {
    let pair = pair();
    assert!(pair.private_key.starts_with("-----BEGIN PRIVATE KEY-----"));
    assert!(pair.public_key.starts_with("-----BEGIN PUBLIC KEY-----"));
    let info = inspect(&pair.private_key).unwrap();
    assert_eq!((info.kind, info.bits), ("private", 2048));
    assert_eq!(info.public_key, pair.public_key);
    assert_eq!(inspect(&pair.public_key).unwrap().kind, "public");
    // PKCS#1 格式
    let private = private_key(&pair.private_key).unwrap();
    let pkcs1 = private.to_pkcs1_pem(LineEnding::LF).unwrap();
    assert!(pkcs1.starts_with("-----BEGIN RSA PRIVATE KEY-----"));
    assert_eq!(inspect(&pkcs1).unwrap().bits, 2048);
    assert_eq!(inspect("nope").unwrap_err().code, "crypto.invalid_key_pem");
    assert_eq!(
        inspect("-----BEGIN ENCRYPTED PRIVATE KEY-----")
            .unwrap_err()
            .code,
        "crypto.encrypted_key"
    );
    assert_eq!(
        generate(GenerateArgs {
            bits: 1000,
            format: KeyFormat::Pkcs8
        })
        .unwrap_err()
        .code,
        "crypto.invalid_key_bits"
    );
}

#[test]
fn encrypts_and_decrypts_with_every_padding() {
    let pair = pair();
    for padding in [Padding::OaepSha256, Padding::OaepSha1, Padding::Pkcs1v15] {
        let sealed = encrypt(cipher(
            &pair.public_key,
            "机密 secret",
            padding,
            Encoding::Utf8,
            Encoding::Base64,
        ))
        .unwrap();
        // 私钥也可以用来加密（取其公钥部分）
        let sealed_with_private = encrypt(cipher(
            &pair.private_key,
            "x",
            padding,
            Encoding::Utf8,
            Encoding::Hex,
        ))
        .unwrap();
        assert_eq!(sealed_with_private.len(), 512);
        let opened = decrypt(cipher(
            &pair.private_key,
            &sealed,
            padding,
            Encoding::Base64,
            Encoding::Utf8,
        ))
        .unwrap();
        assert_eq!(opened, "机密 secret", "{padding:?}");
    }
    let wrong = decrypt(cipher(
        &pair.private_key,
        &"00".repeat(256),
        Padding::OaepSha256,
        Encoding::Hex,
        Encoding::Utf8,
    ));
    assert_eq!(wrong.unwrap_err().code, "crypto.decrypt_failed");
}

#[test]
fn limits_plaintext_length() {
    assert_eq!(max_plaintext(2048, Padding::OaepSha256), 190);
    assert_eq!(max_plaintext(2048, Padding::Pkcs1v15), 245);
    let long = "a".repeat(191);
    let err = encrypt(cipher(
        &pair().public_key,
        &long,
        Padding::OaepSha256,
        Encoding::Utf8,
        Encoding::Base64,
    ))
    .unwrap_err();
    assert_eq!(
        (err.code.as_str(), err.params["max"].as_u64()),
        ("crypto.message_too_long", Some(190))
    );
}

#[test]
fn signs_and_verifies() {
    let pair = pair();
    for scheme in [Scheme::Pkcs1v15, Scheme::Pss] {
        for hash in [Hash::Sha256, Hash::Sha384, Hash::Sha512] {
            let mut args = SignArgs {
                key: pair.private_key.clone(),
                message: "hello".into(),
                message_encoding: Encoding::Utf8,
                scheme,
                hash,
                signature_encoding: Encoding::Base64,
                signature: String::new(),
            };
            let signature = sign(SignArgs {
                key: args.key.clone(),
                message: args.message.clone(),
                signature: String::new(),
                ..args
            })
            .unwrap();
            args.key = pair.public_key.clone();
            args.signature = signature;
            assert!(
                verify(SignArgs {
                    key: args.key.clone(),
                    message: args.message.clone(),
                    signature: args.signature.clone(),
                    ..args
                })
                .unwrap()
            );
            args.message = "hello!".into();
            assert!(!verify(args).unwrap(), "{scheme:?} {hash:?}");
        }
    }
}
