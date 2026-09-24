use super::*;

fn args(input: &str, key: &str, algorithms: Vec<Algorithm>) -> Args {
    Args {
        input: input.into(),
        key: key.into(),
        key_encoding: KeyEncoding::Text,
        algorithms,
        output: OutputEncoding::Hex,
        uppercase: false,
        expected: String::new(),
    }
}

/// RFC 4231 测试用例 2
#[test]
fn matches_rfc_4231_vectors() {
    let report = hmac_text(args(
        "what do ya want for nothing?",
        "Jefe",
        vec![Algorithm::Sha224, Algorithm::Sha256, Algorithm::Sha512],
    ))
    .unwrap();
    assert_eq!(
        report.results[0].mac,
        "a30e01098bc6dbbf45690f3a7e9e6d0f8bbea2a39e6148008fd05e44"
    );
    assert_eq!(
        report.results[1].mac,
        "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
    );
    assert!(
        report.results[2]
            .mac
            .starts_with("164b7a7bfcf819e2e395fbe73b56e0a387bd64222e831fd6")
    );
    assert_eq!(report.key_bytes, 4);
}

/// RFC 2104 / 2202 的 MD5 与 SHA-1 用例
#[test]
fn matches_md5_and_sha1_vectors() {
    let report = hmac_text(args(
        "what do ya want for nothing?",
        "Jefe",
        vec![Algorithm::Md5, Algorithm::Sha1],
    ))
    .unwrap();
    assert_eq!(report.results[0].mac, "750c783e6ab0b503eaa86e310a5db738");
    assert_eq!(
        report.results[1].mac,
        "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79"
    );
}

#[test]
fn key_encodings_and_output() {
    let mut hex_key = args("hi", "4a656665", vec![Algorithm::Sha256]);
    hex_key.key_encoding = KeyEncoding::Hex;
    let mut b64_key = args("hi", "SmVmZQ==", vec![Algorithm::Sha256]);
    b64_key.key_encoding = KeyEncoding::Base64;
    let text_key = args("hi", "Jefe", vec![Algorithm::Sha256]);
    let a = hmac_text(hex_key).unwrap().results[0].mac.clone();
    assert_eq!(a, hmac_text(b64_key).unwrap().results[0].mac);
    assert_eq!(a, hmac_text(text_key).unwrap().results[0].mac);

    let mut b64 = args("hi", "Jefe", vec![Algorithm::Sha256]);
    b64.output = OutputEncoding::Base64;
    let out = hmac_text(b64).unwrap().results[0].mac.clone();
    assert_eq!(
        data_encoding::BASE64.decode(out.as_bytes()).unwrap(),
        hex::decode(&a).unwrap()
    );

    let mut bad = args("hi", "zz", vec![Algorithm::Sha256]);
    bad.key_encoding = KeyEncoding::Hex;
    assert_eq!(hmac_text(bad).unwrap_err().code, "hmac.invalid_key");
}

#[test]
fn verifies_expected_values() {
    let mut a = args(
        "what do ya want for nothing?",
        "Jefe",
        vec![Algorithm::Sha256, Algorithm::Sha1],
    );
    a.expected = "5BDCC146 BF60754E6A042426089575C75A003F089D2739839DEC58B964EC3843".into();
    let report = hmac_text(a).unwrap();
    assert_eq!(report.results[0].matches, Some(true));
    assert_eq!(report.results[1].matches, Some(false));
    // 也接受 Base64 形式
    let mut b = args(
        "what do ya want for nothing?",
        "Jefe",
        vec![Algorithm::Sha256],
    );
    b.expected = "W9zBRr9gdU5qBCQmCJV1x1oAPwidJzmDnexYuWTsOEM=".into();
    assert_eq!(hmac_text(b).unwrap().results[0].matches, Some(true));
    assert!(
        hmac_text(args("x", "k", vec![Algorithm::Sm3]))
            .unwrap()
            .results[0]
            .matches
            .is_none()
    );
}

#[test]
fn crc32_is_skipped() {
    let report = hmac_text(args("x", "k", vec![Algorithm::Crc32, Algorithm::Sm3])).unwrap();
    assert_eq!(report.results.len(), 1);
    assert_eq!(
        hmac_text(args("x", "k", vec![Algorithm::Crc32]))
            .unwrap_err()
            .code,
        "hash.no_algorithms"
    );
}
