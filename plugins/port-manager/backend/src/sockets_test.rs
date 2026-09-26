use std::net::{TcpListener, UdpSocket};

use super::*;

fn socket(protocol: Protocol, port: u16, state: &str) -> Socket {
    Socket {
        protocol,
        local_addr: "127.0.0.1".into(),
        port,
        remote_addr: None,
        remote_port: None,
        state: state.into(),
        pids: vec![1],
        loopback: true,
    }
}

#[test]
fn sorts_listeners_first_and_dedupes() {
    let mut list = vec![
        socket(Protocol::Tcp, 9000, "established"),
        socket(Protocol::Udp, 53, "bound"),
        socket(Protocol::Tcp, 8080, "listen"),
        socket(Protocol::Tcp, 3000, "listen"),
        socket(Protocol::Tcp, 3000, "listen"),
    ];
    sort(&mut list);
    let order: Vec<(u16, &str)> = list.iter().map(|s| (s.port, s.state.as_str())).collect();
    assert_eq!(
        order,
        vec![
            (3000, "listen"),
            (8080, "listen"),
            (53, "bound"),
            (9000, "established")
        ]
    );
}

#[test]
fn maps_ipv4_in_ipv6_addresses() {
    assert_eq!(address("::ffff:10.0.0.1".parse().unwrap()), "10.0.0.1");
    assert_eq!(address("::1".parse().unwrap()), "::1");
}

#[test]
fn finds_own_listeners() {
    let tcp = TcpListener::bind("127.0.0.1:0").unwrap();
    let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
    let (tcp_port, udp_port) = (
        tcp.local_addr().unwrap().port(),
        udp.local_addr().unwrap().port(),
    );
    let me = std::process::id();
    let all = list(false).unwrap();
    let find = |protocol, port| {
        all.iter()
            .find(|s| s.protocol == protocol && s.port == port)
    };
    let listener = find(Protocol::Tcp, tcp_port).expect("tcp listener");
    assert_eq!(listener.state, "listen");
    assert!(
        listener.loopback && listener.pids.contains(&me),
        "{listener:?}"
    );
    assert!(find(Protocol::Udp, udp_port).is_some());
    assert!(
        all.iter()
            .all(|s| s.state == "listen" || s.state == "bound")
    );
}
