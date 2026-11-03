use super::*;
use serde_json::json;

#[test]
fn generates_candidates_with_price_groups() {
    let out = candidates(CandidateArgs {
        brands: vec!["acme".into(), "acme-labs".into()],
        tlds: vec!["com".into(), "io".into(), "dev".into()],
        hyphens: true,
    })
    .unwrap();
    assert!(out.total >= 6);
    let domains: Vec<&str> = out.candidates.iter().map(|c| c.domain.as_str()).collect();
    assert!(domains.contains(&"acme.com"));
    assert!(domains.contains(&"acme-labs.io"));
    assert!(domains.contains(&"acme-get.dev"));
    assert!(
        out.candidates
            .iter()
            .all(|c| c.price == "standard" || c.price == "premium" || c.price == "expensive")
    );
}

#[test]
fn rejects_bad_brand_input() {
    let err = candidates(CandidateArgs {
        brands: vec![],
        tlds: vec![],
        hyphens: false,
    })
    .unwrap_err();
    assert_eq!(err.code, "domain.bad_brands");
    let err = candidates(CandidateArgs {
        brands: vec!["bad domain!".into()],
        tlds: vec![],
        hyphens: false,
    })
    .unwrap_err();
    assert_eq!(err.code, "domain.bad_brands");
}

#[test]
fn dns_website_records() {
    let out = dns(DnsArgs {
        scenario: "website".into(),
        domain: "acme.com".into(),
        ip: Some("203.0.113.10".into()),
        target: Some("shop.example.com".into()),
        mail_host: None,
        dkim_selector: None,
    })
    .unwrap();
    assert_eq!(out.records.len(), 2);
    assert_eq!(out.records[0].record_type, "A");
    assert_eq!(out.records[1].value, "shop.example.com.");
    let err = dns(DnsArgs {
        scenario: "website".into(),
        domain: "acme.com".into(),
        ip: Some("999.1.1.1".into()),
        target: None,
        mail_host: None,
        dkim_selector: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "domain.bad_ip");
}

#[test]
fn dns_email_generates_trio() {
    let out = dns(DnsArgs {
        scenario: "email".into(),
        domain: "acme.com".into(),
        ip: None,
        target: None,
        mail_host: None,
        dkim_selector: None,
    })
    .unwrap();
    let types: Vec<&str> = out.records.iter().map(|r| r.record_type).collect();
    assert!(types.contains(&"MX"));
    assert_eq!(
        types.iter().filter(|t| **t == "TXT").count(),
        3,
        "SPF + DKIM + DMARC"
    );
    let spf = out
        .records
        .iter()
        .find(|r| r.value.starts_with("v=spf1"))
        .unwrap();
    assert!(spf.value.ends_with("-all"));
    let dmarc = out
        .records
        .iter()
        .find(|r| r.value.starts_with("v=DMARC1"))
        .unwrap();
    assert!(dmarc.name == "_dmarc");
}

#[test]
fn validates_spf() {
    let out = validate_spf_record(RecordArgs {
        record: "v=spf1 include:_spf.acme.com -all".into(),
    })
    .unwrap();
    assert!(out.valid);
    assert_eq!(out.policy, "hard");
    assert_eq!(out.includes, vec!["_spf.acme.com".to_string()]);
    let out = validate_spf_record(RecordArgs {
        record: "v=spf1 mx ~all".into(),
    })
    .unwrap();
    assert!(out.valid);
    assert_eq!(out.policy, "soft");
    let out = validate_spf_record(RecordArgs {
        record: "not-spf".into(),
    })
    .unwrap();
    assert!(!out.valid);
    let out = validate_spf_record(RecordArgs {
        record: "v=spf1 include:bad..com -all".into(),
    })
    .unwrap();
    assert!(!out.valid, "double dot domain is invalid");
    let out = validate_spf_record(RecordArgs {
        record: "v=spf1 ip4:203.0.113.0/24 -all".into(),
    })
    .unwrap();
    assert!(out.valid);
}

#[test]
fn validates_dmarc() {
    let out = validate_dmarc(RecordArgs {
        record: "v=DMARC1; p=quarantine; rua=mailto:d@acme.com; pct=100".into(),
    })
    .unwrap();
    assert!(out.valid);
    assert_eq!(out.policy, "quarantine");
    assert_eq!(out.reports, vec!["d@acme.com".to_string()]);
    let out = validate_dmarc(RecordArgs {
        record: "v=DMARC1; p=bogus".into(),
    })
    .unwrap();
    assert!(!out.valid);
    let out = validate_dmarc(RecordArgs {
        record: "v=DMARC1; p=reject; pct=200".into(),
    })
    .unwrap();
    assert!(!out.valid, "pct over 100 is rejected");
}

#[test]
fn dispatches_via_plugin() {
    let tool = DomainToolbox::default();
    let out = tool
        .call("candidates", json!({"brands": ["acme"], "tlds": ["com"]}))
        .unwrap();
    assert_eq!(out["total"], 1);
    let out = tool
        .call("validate_spf", json!({"record": "v=spf1 -all"}))
        .unwrap();
    assert_eq!(out["valid"], true);
    assert_eq!(
        tool.call("nope", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&DomainToolbox::default()).unwrap();
    assert_eq!(checked, 4);
}
