use serde_json::json;

use super::*;

const CHAIN: &str = include_str!("../fixtures/chain.pem");

#[test]
fn parses_text_and_files() {
    let tool = CertificateViewer::default();
    let out = tool.call("parse", json!({ "text": CHAIN })).unwrap();
    assert_eq!(out["certificates"].as_array().unwrap().len(), 2);
    assert_eq!(out["ordered"], true);
    assert_eq!(
        out["certificates"][0]["subject"]["commonName"],
        "example.test"
    );

    let der = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/leaf.der");
    let out = tool.call("parse_file", json!({ "path": der })).unwrap();
    assert_eq!(out["certificates"][0]["serial"], "12:34:AB:CD");
    assert_eq!(
        tool.call("parse_file", json!({ "path": "/nope.pem" }))
            .unwrap_err()
            .code,
        "fs.not_found"
    );
    assert_eq!(
        tool.call("parse", json!({ "text": "hello" }))
            .unwrap_err()
            .code,
        "cert.not_a_certificate"
    );
}

#[test]
fn detects_pem_text() {
    let tool = CertificateViewer::default();
    assert_eq!(
        tool.detect(CHAIN).unwrap(),
        Detection::new(98, "pem").with("count", 2)
    );
    assert!(tool.detect("-----BEGIN PRIVATE KEY-----").is_none());
    assert!(tool.manifest().functions["fetch"].task);
}
