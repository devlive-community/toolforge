use super::*;

#[test]
fn normalizes_query_names() {
    assert_eq!(
        query_name("example.com", RecordType::A)
            .unwrap()
            .to_string(),
        "example.com."
    );
    assert_eq!(
        query_name("https://www.example.com:8443/path?q=1", RecordType::A)
            .unwrap()
            .to_string(),
        "www.example.com."
    );
    assert_eq!(
        query_name("8.8.8.8", RecordType::PTR).unwrap().to_string(),
        "8.8.8.8.in-addr.arpa."
    );
    assert!(
        query_name("2001:db8::1", RecordType::PTR)
            .unwrap()
            .to_string()
            .ends_with("ip6.arpa.")
    );
    assert_eq!(
        query_name("8.8.8.8", RecordType::A).unwrap_err().code,
        "dns.ip_needs_ptr"
    );
    assert_eq!(
        query_name("  ", RecordType::A).unwrap_err().code,
        "dns.empty"
    );
}

#[test]
fn validates_types_and_servers() {
    assert_eq!(record_type("mx").unwrap(), RecordType::MX);
    assert_eq!(
        record_type("AXFR").unwrap_err().code,
        "dns.unsupported_type"
    );
    assert_eq!(server_address("system").unwrap(), None);
    assert_eq!(
        server_address("cloudflare").unwrap().unwrap().to_string(),
        "1.1.1.1"
    );
    assert_eq!(
        server_address("192.168.1.1").unwrap().unwrap().to_string(),
        "192.168.1.1"
    );
    assert_eq!(
        server_address("nope").unwrap_err().code,
        "dns.invalid_server"
    );
    let err = lookup(Args {
        name: "example.com".into(),
        record_type: "A".into(),
        servers: vec![" ".into()],
    })
    .unwrap_err();
    assert_eq!(err.code, "dns.no_servers");
}

#[test]
fn detects_fake_ips() {
    assert!(is_fake_ip("198.18.2.153"));
    assert!(is_fake_ip("198.19.255.1"));
    assert!(!is_fake_ip("198.20.0.1"));
    assert!(!is_fake_ip("1.1.1.1"));
    assert!(!is_fake_ip("mail.example.com."));
}

#[test]
fn mx_records_sort_by_priority() {
    let answer = |value: &str| Answer {
        name: "gmail.com.".into(),
        record_type: "MX".into(),
        ttl: 300,
        value: value.into(),
        fake_ip: false,
    };
    let mut list = vec![answer("40 alt4."), answer("5 main."), answer("10 alt1.")];
    sort_answers(&mut list);
    assert_eq!(
        list.iter().map(|a| a.value.as_str()).collect::<Vec<_>>(),
        vec!["5 main.", "10 alt1.", "40 alt4."]
    );
}

#[test]
fn invalid_servers_fail_individually() {
    let report = lookup(Args {
        name: "example.com".into(),
        record_type: "A".into(),
        servers: vec!["bogus".into()],
    })
    .unwrap();
    assert_eq!(
        report.results[0].error.as_ref().unwrap().code,
        "dns.invalid_server"
    );
}

/// 需要网络：`cargo test -p tfp-dns-lookup -- --ignored`
/// 使用 TUN / fake-ip 代理的机器会拿到 198.18.x.x，因此只断言记录类型与结构
#[test]
#[ignore]
fn resolves_against_public_servers() {
    let report = lookup(Args {
        name: "one.one.one.one".into(),
        record_type: "A".into(),
        servers: vec!["system".into(), "cloudflare".into(), "google".into()],
    })
    .unwrap();
    assert_eq!(report.results.len(), 3);
    for result in &report.results {
        assert!(
            result.error.is_none(),
            "{}: {:?}",
            result.server,
            result.error
        );
        assert!(!result.answers.is_empty());
        assert!(
            result
                .answers
                .iter()
                .all(|a| a.record_type == "A" && a.value.parse::<IpAddr>().is_ok())
        );
    }
    let mx = lookup(Args {
        name: "gmail.com".into(),
        record_type: "MX".into(),
        servers: vec!["cloudflare".into()],
    })
    .unwrap();
    assert!(
        mx.results[0].answers.iter().all(|a| a.record_type == "MX"),
        "{:?}",
        mx.results[0]
    );
}
