use super::*;

#[test]
fn filters_and_ranks_lan_addresses() {
    let ip = |s: &str| s.parse::<Ipv4Addr>().unwrap();
    assert!(usable_lan(&ip("192.168.1.8")));
    assert!(!usable_lan(&ip("127.0.0.1")));
    assert!(!usable_lan(&ip("169.254.3.4")));
    assert!(!usable_lan(&ip("198.18.0.1")) && !usable_lan(&ip("198.19.255.1")));
    assert!(usable_lan(&ip("198.20.0.1")));
    assert!(rank(&ip("192.168.0.2")) < rank(&ip("10.0.0.2")));
    assert!(rank(&ip("172.20.0.1")) < rank(&ip("100.64.0.1")));
    assert!(is_virtual("bridge100") && is_virtual("vEthernet (WSL)") && is_virtual("docker0"));
    assert!(!is_virtual("en0") && !is_virtual("eth0") && !is_virtual("Wi-Fi"));
}

#[test]
fn reports_addresses_and_port_availability() {
    let taken = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = taken.local_addr().unwrap().port();
    let result = addresses(Args { port, lan: false }).unwrap();
    assert!(!result.available);
    assert_eq!(result.addresses.len(), 1);
    assert_eq!(result.addresses[0].url, format!("http://localhost:{port}/"));
    assert_eq!(bind(port, false).unwrap_err().code, "server.port_in_use");
    assert_eq!(bind(0, false).unwrap_err().code, "server.invalid_port");

    let free = free_port(port, false).unwrap();
    assert_ne!(free, port);
    for lan in addresses(Args {
        port: free,
        lan: true,
    })
    .unwrap()
    .addresses
    .iter()
    .skip(1)
    {
        assert_eq!(lan.kind, "lan");
        assert!(
            lan.qr
                .as_deref()
                .is_some_and(|qr| qr.starts_with("data:image/svg+xml;base64,"))
        );
    }
}

#[test]
fn renders_qr_codes() {
    let image = qr_image("http://192.168.1.8:8000/").unwrap();
    let encoded = image.strip_prefix("data:image/svg+xml;base64,").unwrap();
    let svg = String::from_utf8(data_encoding::BASE64.decode(encoded.as_bytes()).unwrap()).unwrap();
    assert!(svg.contains("<svg") && svg.contains("#ffffff"), "{svg}");
}
