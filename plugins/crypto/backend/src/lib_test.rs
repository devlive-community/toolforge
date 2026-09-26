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
}
