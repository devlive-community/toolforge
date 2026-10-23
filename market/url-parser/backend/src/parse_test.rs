use super::*;

#[test]
fn splits_every_part() {
    let p = parse(
        "https://user:p%40ss@example.com:8443/a%20b/c/?q=hello+world&tag=%E4%B8%AD&tag=x#sec%201",
    )
    .unwrap();
    assert_eq!(p.scheme, "https");
    assert_eq!(
        (p.username.as_str(), p.password.as_deref()),
        ("user", Some("p@ss"))
    );
    assert_eq!(p.host.as_deref(), Some("example.com"));
    assert_eq!(p.host_kind, Some(HostKind::Domain));
    assert_eq!((p.port, p.default_port), (Some(8443), Some(443)));
    assert_eq!(p.origin, "https://example.com:8443");
    assert_eq!(p.path, "/a b/c/");
    assert_eq!(p.segments, ["a b", "c"]);
    assert_eq!(
        p.params[0],
        ParamInfo {
            key: "q".into(),
            value: "hello world".into(),
            raw: "q=hello+world".into(),
            tracking: false,
            duplicate: false
        }
    );
    assert_eq!(p.params[1].value, "中");
    assert!(p.params[1].duplicate && p.params[2].duplicate);
    assert_eq!(p.fragment.as_deref(), Some("sec 1"));
    assert_eq!(
        p.readable,
        "https://user@example.com:8443/a b/c/?q=hello world&tag=中&tag=x#sec 1"
    );
    assert!(!p.assumed_scheme);
}

#[test]
fn assumes_https_only_without_a_scheme() {
    let p = parse("example.com/path?a=1").unwrap();
    assert!(p.assumed_scheme);
    assert_eq!(p.href, "https://example.com/path?a=1");
    let local = parse("localhost:8080/api").unwrap();
    assert!(local.assumed_scheme);
    assert_eq!(
        (local.host.as_deref(), local.port),
        (Some("localhost"), Some(8080))
    );
    let mail = parse("mailto:someone@example.com").unwrap();
    assert!(!mail.assumed_scheme);
    assert_eq!(mail.host, None);
    assert_eq!(mail.path, "someone@example.com");
}

#[test]
fn recognises_hosts() {
    let idn = parse("https://例子.测试/").unwrap();
    assert_eq!(idn.host.as_deref(), Some("xn--fsqu00a.xn--0zwm56d"));
    assert_eq!(idn.host_unicode.as_deref(), Some("例子.测试"));
    assert!(idn.readable.starts_with("https://例子.测试/"));
    let v4 = parse("http://192.168.1.10/").unwrap();
    assert_eq!(
        (v4.host_kind, v4.port, v4.default_port),
        (Some(HostKind::Ipv4), None, Some(80))
    );
    let v6 = parse("http://[::1]:3000/").unwrap();
    assert_eq!(v6.host_kind, Some(HostKind::Ipv6));
    assert_eq!(v6.host.as_deref(), Some("::1"));
}

#[test]
fn reports_bad_input() {
    assert_eq!(parse("  ").unwrap_err().code, "url.empty");
    assert_eq!(
        parse("http://exa mple.com").unwrap_err().code,
        "url.invalid"
    );
    assert_eq!(parse("https://").unwrap_err().code, "url.invalid");
}
