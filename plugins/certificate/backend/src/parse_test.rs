use super::*;

const LEAF: &str = include_str!("../fixtures/leaf.pem");
const CA: &str = include_str!("../fixtures/ca.pem");
const CHAIN: &str = include_str!("../fixtures/chain.pem");
const LEAF_DER: &[u8] = include_bytes!("../fixtures/leaf.der");
/// 2027-01-15，位于测试证书的有效期内（证书生成于 2026-09-25，有效 100 年）
const NOW: i64 = 1_800_000_000;

fn leaf() -> Certificate {
    certificate(&split(LEAF.as_bytes()).unwrap()[0], NOW).unwrap()
}

#[test]
fn splits_every_input_form() {
    assert_eq!(split(CHAIN.as_bytes()).unwrap().len(), 2);
    assert_eq!(split(LEAF_DER).unwrap()[0], LEAF_DER);
    let base64 = data_encoding::BASE64.encode(LEAF_DER);
    assert_eq!(split(base64.as_bytes()).unwrap()[0], LEAF_DER);
    assert_eq!(split(b"hello").unwrap_err().code, "cert.not_a_certificate");
    assert_eq!(split(b"").unwrap_err().code, "cert.not_a_certificate");
    let many = LEAF.repeat(MAX_CERTIFICATES + 1);
    assert_eq!(split(many.as_bytes()).unwrap_err().code, "cert.too_many");
}

#[test]
fn reads_names_validity_and_key() {
    let cert = leaf();
    assert_eq!(cert.subject.common_name.as_deref(), Some("example.test"));
    assert_eq!(cert.subject.organization.as_deref(), Some("Example Org"));
    assert_eq!(
        cert.issuer.common_name.as_deref(),
        Some("ToolForge Test CA")
    );
    assert_eq!(cert.serial, "12:34:AB:CD");
    assert_eq!(cert.version, 3);
    assert_eq!(cert.validity, "valid");
    assert!(cert.days_left > 30_000);
    assert_eq!(
        cert.key,
        Key {
            algorithm: "RSA".into(),
            bits: Some(2048),
            curve: None
        }
    );
    assert_eq!(cert.signature_algorithm, "SHA256withECDSA");
    assert!(!cert.self_signed && !cert.is_ca);
    assert_eq!(
        cert.fingerprints.sha256,
        "E0:70:D3:0E:3B:FD:A2:7C:BF:9C:8F:4C:9C:40:3A:31:B8:6E:07:43:AB:D1:8E:98:43:AE:45:72:53:A8:53:14"
    );
    assert!(cert.pem.starts_with("-----BEGIN CERTIFICATE-----\n"));
    assert_eq!(split(cert.pem.as_bytes()).unwrap()[0], LEAF_DER);
}

#[test]
fn reads_extensions() {
    let cert = leaf();
    let alt: Vec<(&str, &str)> = cert
        .alt_names
        .iter()
        .map(|a| (a.kind, a.value.as_str()))
        .collect();
    assert_eq!(
        alt,
        vec![
            ("dns", "example.test"),
            ("dns", "*.example.test"),
            ("ip", "127.0.0.1"),
            ("email", "ops@example.test")
        ]
    );
    assert_eq!(cert.extended_key_usage, vec!["serverAuth", "clientAuth"]);
    assert!(
        cert.key_usage.iter().any(|u| u.contains("Digital")),
        "{:?}",
        cert.key_usage
    );
    assert_eq!(cert.ocsp, vec!["http://ocsp.example.test"]);
    assert_eq!(cert.ca_issuers, vec!["http://ca.example.test/ca.crt"]);
    assert_eq!(cert.crl, vec!["http://crl.example.test/ca.crl"]);
    assert!(cert.subject_key_id.is_some() && cert.authority_key_id.is_some());
}

#[test]
fn reads_the_ca_and_orders_chains() {
    let ca = certificate(&split(CA.as_bytes()).unwrap()[0], NOW).unwrap();
    assert!(ca.is_ca && ca.self_signed);
    assert_eq!(ca.path_len, Some(0));
    assert_eq!(
        ca.key,
        Key {
            algorithm: "EC".into(),
            bits: Some(256),
            curve: Some("P-256".into())
        }
    );
    assert_eq!(ca.authority_key_id, ca.subject_key_id);
    let leaf = leaf();
    assert_eq!(leaf.authority_key_id, ca.subject_key_id);
    let chain = vec![leaf, ca];
    assert!(chain_ordered(&chain));
    let reversed: Vec<Certificate> = chain.into_iter().rev().collect();
    assert!(!chain_ordered(&reversed));
}

#[test]
fn reports_validity_against_the_clock() {
    let der = &split(LEAF.as_bytes()).unwrap()[0];
    assert_eq!(certificate(der, 0).unwrap().validity, "not_yet_valid");
    let expired = certificate(der, i64::MAX / 2).unwrap();
    assert_eq!(expired.validity, "expired");
    assert!(expired.days_left < 0);
}
