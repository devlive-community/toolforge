use super::*;

#[test]
fn parses_win_proxy_lists_and_host_ports() {
    let parsed = parse_win_proxy_list("proxy.corp:8080");
    assert_eq!(parsed[0], ("http", "proxy.corp".to_owned(), 8080));

    let parsed = parse_win_proxy_list("http=10.0.0.1:80;https=10.0.0.1:443");
    assert_eq!(parsed[0], ("http", "10.0.0.1".to_owned(), 80));
    assert_eq!(parsed[1], ("https", "10.0.0.1".to_owned(), 443));

    let parsed = parse_win_proxy_list("[::1]:8080");
    assert_eq!(parsed[0], ("http", "::1".to_owned(), 8080));

    let parsed = parse_win_proxy_list("socks=relay.corp:1080;ftp=old:21");
    assert_eq!(parsed[0], ("socks", "relay.corp".to_owned(), 1080));
    assert_eq!(parsed[1], ("ftp", "old".to_owned(), 21));

    // 无端口、空段、裸 IPv6 都安全跳过
    assert!(parse_win_proxy_list("").is_empty());
    assert!(parse_win_proxy_list("no-port-here").is_empty());
    assert!(parse_host_port("2001:db8::1").is_none());
    // 端口非法：解析失败返回 None（非法值通过条目级校验提示）
    assert!(parse_host_port("host:notaport").is_none());
}
