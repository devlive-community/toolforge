use super::*;

fn calc(input: &str) -> Report {
    calculate(Args {
        input: input.into(),
        split: None,
        contains: None,
    })
    .unwrap()
}

#[test]
fn ipv4_cidr() {
    let r = calc("192.168.1.130/26");
    assert_eq!(r.version, 4);
    assert_eq!(r.cidr, "192.168.1.128/26");
    assert_eq!(r.network, "192.168.1.128");
    assert_eq!(r.broadcast.as_deref(), Some("192.168.1.191"));
    assert_eq!(r.netmask, "255.255.255.192");
    assert_eq!(r.wildcard, "0.0.0.63");
    assert_eq!(
        (r.first_host.as_str(), r.last_host.as_str()),
        ("192.168.1.129", "192.168.1.190")
    );
    assert_eq!((r.total.as_str(), r.usable.as_str()), ("64", "62"));
    assert_eq!((r.kind, r.class), ("private", Some('C')));
    assert_eq!(r.binary, "11000000.10101000.00000001.10000010");
    assert_eq!(r.hex, "0xC0A80182");
    assert_eq!(r.integer, "3232235906");
    assert_eq!(r.reverse_dns, "130.1.168.192.in-addr.arpa");
    assert_eq!(r.expanded, "0000:0000:0000:0000:0000:ffff:c0a8:0182");
}

#[test]
fn netmask_forms_and_host_routes() {
    assert_eq!(calc("10.1.2.3 255.255.0.0").cidr, "10.1.0.0/16");
    assert_eq!(calc("10.1.2.3/255.255.255.0").prefix, 24);
    let single = calc("8.8.8.8");
    assert_eq!(
        (single.prefix, single.usable.as_str(), single.kind),
        (32, "1", "global")
    );
    // /31 点对点链路两个地址都可用
    let p2p = calc("10.0.0.0/31");
    assert_eq!(
        (
            p2p.first_host.as_str(),
            p2p.last_host.as_str(),
            p2p.usable.as_str()
        ),
        ("10.0.0.0", "10.0.0.1", "2")
    );
    let all = calc("0.0.0.0/0");
    assert_eq!(
        (all.total.as_str(), all.netmask.as_str()),
        ("4294967296", "0.0.0.0")
    );
}

#[test]
fn address_kinds() {
    let kinds: Vec<_> = [
        "127.0.0.1",
        "169.254.1.1",
        "224.0.0.1",
        "100.64.0.1",
        "192.0.2.10",
        "240.0.0.1",
        "::1",
        "fe80::1",
        "fd00::1",
        "2001:db8::1",
        "2606:4700::1111",
    ]
    .iter()
    .map(|ip| calc(ip).kind)
    .collect();
    assert_eq!(
        kinds,
        vec![
            "loopback",
            "link_local",
            "multicast",
            "shared",
            "documentation",
            "reserved",
            "loopback",
            "link_local",
            "unique_local",
            "documentation",
            "global"
        ]
    );
}

#[test]
fn ipv6_prefix() {
    let r = calc("2001:db8:abcd:12::1/48");
    assert_eq!(r.version, 6);
    assert_eq!(r.cidr, "2001:db8:abcd::/48");
    assert!(r.broadcast.is_none() && r.class.is_none());
    assert_eq!(r.last_host, "2001:db8:abcd:ffff:ffff:ffff:ffff:ffff");
    assert_eq!(r.total, "1208925819614629174706176");
    assert_eq!(r.netmask, "ffff:ffff:ffff::");
    assert_eq!(r.expanded, "2001:0db8:abcd:0012:0000:0000:0000:0001");
    assert!(
        r.reverse_dns.starts_with("1.0.0.0.")
            && r.reverse_dns.ends_with(".8.b.d.0.1.0.0.2.ip6.arpa")
    );
    assert_eq!(
        calc("::/0").total,
        "340282366920938463463374607431768211456"
    );
}

#[test]
fn splits_subnets() {
    let r = calculate(Args {
        input: "10.0.0.0/24".into(),
        split: Some(26),
        contains: None,
    })
    .unwrap();
    assert_eq!(r.subnet_count.as_deref(), Some("4"));
    assert_eq!(
        r.subnets
            .iter()
            .map(|s| s.cidr.as_str())
            .collect::<Vec<_>>(),
        vec![
            "10.0.0.0/26",
            "10.0.0.64/26",
            "10.0.0.128/26",
            "10.0.0.192/26"
        ]
    );
    assert_eq!(r.subnets[1].last, "10.0.0.127");

    let many = calculate(Args {
        input: "10.0.0.0/8".into(),
        split: Some(24),
        contains: None,
    })
    .unwrap();
    assert_eq!(
        (many.subnets.len(), many.subnet_count.as_deref()),
        (256, Some("65536"))
    );

    let err = calculate(Args {
        input: "10.0.0.0/24".into(),
        split: Some(16),
        contains: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "ip.invalid_split");
}

#[test]
fn checks_membership() {
    let check = |other: &str| {
        calculate(Args {
            input: "172.16.0.0/12".into(),
            split: None,
            contains: Some(other.into()),
        })
        .unwrap()
        .membership
        .unwrap()
        .inside
    };
    assert!(check("172.31.255.255"));
    assert!(!check("172.32.0.1"));
    assert!(!check("::1"));
}

#[test]
fn reports_bad_input() {
    let err = |input: &str| {
        calculate(Args {
            input: input.into(),
            split: None,
            contains: None,
        })
        .unwrap_err()
        .code
    };
    assert_eq!(err(""), "ip.empty");
    assert_eq!(err("300.1.1.1"), "ip.invalid");
    assert_eq!(err("10.0.0.1/33"), "ip.invalid_prefix");
    assert_eq!(err("10.0.0.1/255.0.255.0"), "ip.invalid_mask");
    assert_eq!(err("::1/255.255.0.0"), "ip.invalid_prefix");
}

#[test]
fn converts_ranges_to_cidrs() {
    let r = range_to_cidrs(RangeArgs {
        start: "10.0.0.5".into(),
        end: "10.0.0.20".into(),
    })
    .unwrap();
    assert_eq!(
        r.cidrs,
        vec![
            "10.0.0.5/32",
            "10.0.0.6/31",
            "10.0.0.8/29",
            "10.0.0.16/30",
            "10.0.0.20/32"
        ]
    );
    assert_eq!(r.total, "16");

    let whole = range_to_cidrs(RangeArgs {
        start: "0.0.0.0".into(),
        end: "255.255.255.255".into(),
    })
    .unwrap();
    assert_eq!(whole.cidrs, vec!["0.0.0.0/0"]);

    let v6 = range_to_cidrs(RangeArgs {
        start: "::".into(),
        end: "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff".into(),
    })
    .unwrap();
    assert_eq!(
        (v6.cidrs, v6.total.as_str()),
        (
            vec!["::/0".to_owned()],
            "340282366920938463463374607431768211456"
        )
    );

    assert_eq!(
        range_to_cidrs(RangeArgs {
            start: "10.0.0.9".into(),
            end: "10.0.0.1".into()
        })
        .unwrap_err()
        .code,
        "ip.range_reversed"
    );
    assert_eq!(
        range_to_cidrs(RangeArgs {
            start: "10.0.0.1".into(),
            end: "::1".into()
        })
        .unwrap_err()
        .code,
        "ip.version_mismatch"
    );
}
