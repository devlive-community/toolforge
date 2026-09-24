use super::*;

#[test]
fn lists_servers_and_types() {
    let out = DnsLookup::default().call("servers", json!({})).unwrap();
    assert_eq!(out["types"][0], "A");
    assert_eq!(out["public"][0]["id"], "cloudflare");
}
