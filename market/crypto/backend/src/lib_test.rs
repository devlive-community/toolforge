use serde_json::json;

use super::*;

#[test]
fn exposes_functions_and_random_bytes() {
    let tool = Crypto::default();
    let key = tool
        .call("random", json!({ "length": 32, "encoding": "hex" }))
        .unwrap();
    assert_eq!(key.as_str().unwrap().len(), 64);
    assert_ne!(
        key,
        tool.call("random", json!({ "length": 32, "encoding": "hex" }))
            .unwrap()
    );
    assert_eq!(
        tool.call("random", json!({ "length": 0, "encoding": "hex" }))
            .unwrap_err()
            .code,
        "crypto.invalid_length"
    );
    let out = tool
        .call(
            "symmetric",
            json!({
                "algorithm": "aes", "mode": "gcm", "direction": "encrypt",
                "input": "hi", "inputEncoding": "utf8", "outputEncoding": "base64",
                "key": "00112233445566778899aabbccddeeff", "keyEncoding": "hex", "keyBits": 128
            }),
        )
        .unwrap();
    assert_eq!(out["iv"].as_str().unwrap().len(), 24);
    assert!(tool.manifest().functions["rsa_generate"].task);
    assert!(tool.manifest().sensitive);
}

#[test]
fn detects_pem_keys_but_not_certificates() {
    assert_eq!(
        detect_key("-----BEGIN PRIVATE KEY-----\nabc")
            .unwrap()
            .label,
        "privateKey"
    );
    assert_eq!(
        detect_key("-----BEGIN RSA PUBLIC KEY-----").unwrap().label,
        "publicKey"
    );
    assert!(detect_key("-----BEGIN CERTIFICATE-----").is_none());
    let sm2 = sm2_ops::generate().unwrap();
    assert_eq!(detect_key(&sm2.private_key).unwrap().label, "sm2PrivateKey");
    assert_eq!(detect_key(&sm2.public_key).unwrap().label, "sm2PublicKey");
}

#[test]
fn exposes_sm2_functions() {
    let tool = Crypto::default();
    let pair = tool.call("sm2_generate", json!({})).unwrap();
    let base = json!({
        "userId": "1234567812345678", "userIdEncoding": "utf8",
        "message": "hi", "messageEncoding": "utf8",
    });
    let mut sign = base.clone();
    sign["key"] = pair["privateKey"].clone();
    sign["format"] = json!("der");
    sign["encoding"] = json!("base64");
    let signed = tool.call("sm2_sign", sign).unwrap();
    assert_eq!(signed["format"], "der");
    assert_eq!(signed["details"]["entla"], 128);
    let mut verify = base;
    verify["key"] = pair["publicKey"].clone();
    verify["signature"] = signed["signature"].clone();
    verify["format"] = json!("auto");
    verify["encoding"] = json!("base64");
    assert_eq!(tool.call("sm2_verify", verify).unwrap()["valid"], true);
    let info = tool
        .call("sm2_inspect", json!({ "key": pair["privateKey"] }))
        .unwrap();
    assert_eq!(
        (info["kind"].as_str(), info["format"].as_str()),
        (Some("private"), Some("pkcs8-pem"))
    );
    let converted = tool
        .call(
            "sm2_convert",
            json!({
                "signature": signed["signature"], "from": "auto", "encoding": "base64",
                "to": "raw", "outputEncoding": "hex"
            }),
        )
        .unwrap();
    assert_eq!(converted["signature"].as_str().unwrap().len(), 128);
    let exported = tool
        .call("sm2_export", json!({ "key": pair["privateKey"] }))
        .unwrap();
    assert_eq!(exported, pair["privateKey"]);
}
