use super::*;
#[test]
fn calculates_ipv4_boundaries() {
    let n = network(parse_cidr("192.168.1.42/24").unwrap());
    assert_eq!(
        (
            n.cidr.as_str(),
            n.broadcast.as_str(),
            n.first_host.as_str(),
            n.last_host.as_str(),
            n.usable_hosts.as_str()
        ),
        (
            "192.168.1.0/24",
            "192.168.1.255",
            "192.168.1.1",
            "192.168.1.254",
            "254"
        )
    );
    let n = network(parse_cidr("0.0.0.0/0").unwrap());
    assert_eq!(
        (
            n.addresses.as_str(),
            n.usable_hosts.as_str(),
            n.broadcast.as_str()
        ),
        ("4294967296", "4294967294", "255.255.255.255")
    );
    assert_eq!(
        network(parse_cidr("255.255.255.255/31").unwrap()).usable_hosts,
        "2"
    );
    assert_eq!(
        network(parse_cidr("255.255.255.255/32").unwrap()).first_host,
        "255.255.255.255"
    );
}
#[test]
fn calculates_ipv6_boundaries() {
    let n = network(parse_cidr("2001:db8:1234:5678::abcd/64").unwrap());
    assert_eq!(
        (
            n.cidr.as_str(),
            n.first_host.as_str(),
            n.last_host.as_str(),
            n.addresses.as_str()
        ),
        (
            "2001:db8:1234:5678::/64",
            "2001:db8:1234:5678::",
            "2001:db8:1234:5678:ffff:ffff:ffff:ffff",
            "18446744073709551616"
        )
    );
    let n = network(parse_cidr("::1/0").unwrap());
    assert_eq!(n.addresses, TWO_TO_128);
    assert_eq!(n.broadcast, "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff");
    assert_eq!(network(parse_cidr("::1/128").unwrap()).usable_hosts, "1");
}
#[test]
fn validates_and_pages_splits() {
    for input in [
        "192.168.1.1",
        "192.168.1.256/24",
        "1.2.3.4/33",
        "::1/129",
        "1.2.3.4/-1",
    ] {
        assert!(parse_cidr(input).is_err());
    }
    let s = split(SplitArgs {
        cidr: "10.1.2.3/16".into(),
        prefix: 24,
        page: 1,
    })
    .unwrap();
    assert_eq!(s.total, "256");
    assert_eq!(s.subnets[0].cidr, "10.1.64.0/24");
    assert_eq!(s.subnets[63].cidr, "10.1.127.0/24");
    let s = split(SplitArgs {
        cidr: "2001:db8::/120".into(),
        prefix: 128,
        page: 3,
    })
    .unwrap();
    assert_eq!(s.subnets[0].cidr, "2001:db8::c0/128");
    assert!(!s.has_next);
    let s = split(SplitArgs {
        cidr: "::/0".into(),
        prefix: 128,
        page: 0,
    })
    .unwrap();
    assert_eq!(s.total, TWO_TO_128);
    assert!(s.has_next);
    assert_eq!(
        split(SplitArgs {
            cidr: "::/0".into(),
            prefix: 0,
            page: 0
        })
        .unwrap()
        .subnets[0]
            .cidr,
        "::/0"
    );
    assert!(
        split(SplitArgs {
            cidr: "10.0.0.0/24".into(),
            prefix: 23,
            page: 0
        })
        .is_err()
    );
    assert!(
        split(SplitArgs {
            cidr: "::/128".into(),
            prefix: 128,
            page: 1
        })
        .is_err()
    );
}
