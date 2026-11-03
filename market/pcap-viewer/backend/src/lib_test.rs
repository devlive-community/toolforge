use super::*;
use serde_json::json;

/// 构造最小 PCAP：头 + 一个含 TCP SYN 的以太网帧
fn build_pcap(big_endian: bool, nanosecond: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let magic: u32 = match (big_endian, nanosecond) {
        (true, false) => 0xa1b2c3d4,
        (false, false) => 0xd4c3b1a2,
        (true, true) => 0xa1b23c4d,
        (false, true) => 0x4d3cb2a1,
    };
    out.extend_from_slice(&magic.to_be_bytes());
    out.extend_from_slice(&[0, 2, 0, 4]); // version
    out.extend_from_slice(&[0, 0, 0, 0]); // thiszone
    out.extend_from_slice(&[0, 0, 0, 0]); // sigfigs
    let write_header_u32 = |out: &mut Vec<u8>, v: u32| {
        if big_endian {
            out.extend_from_slice(&v.to_be_bytes());
        } else {
            out.extend_from_slice(&v.to_le_bytes());
        }
    };
    write_header_u32(&mut out, 0x0000ffff); // snaplen
    write_header_u32(&mut out, 1); // ethernet
    // 帧头：ts=1000.500
    let (sec, usec) = (1000u32, 500u32);
    let write_u32 = |out: &mut Vec<u8>, v: u32| {
        if big_endian {
            out.extend_from_slice(&v.to_be_bytes());
        } else {
            out.extend_from_slice(&v.to_le_bytes());
        }
    };
    write_u32(&mut out, sec);
    let _ = write_u32;
    write_u32(&mut out, usec);
    // ethernet frame: 14 头 + 20 IP + 20 TCP = 54
    let mut frame = Vec::new();
    frame.extend_from_slice(&[0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
    frame.extend_from_slice(&[0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb]);
    frame.extend_from_slice(&[0x08, 0x00]); // IPv4
    frame.extend_from_slice(&[0x45, 0x00, 0x00, 0x28]); // IHL 20, total 40
    frame.extend_from_slice(&[0, 0, 0, 0]);
    frame.extend_from_slice(&[64, 6, 0, 0]); // ttl 64, proto TCP
    frame.extend_from_slice(&[10, 0, 0, 1]); // src
    frame.extend_from_slice(&[93, 184, 216, 34]); // dst
    frame.extend_from_slice(&[0x1f, 0x90]); // 8080
    frame.extend_from_slice(&[0x00, 0x50]); // 80
    frame.extend_from_slice(&[0, 0, 0, 1]);
    frame.extend_from_slice(&[0, 0, 0, 2]);
    frame.extend_from_slice(&[0x50, 0x02, 0x20, 0x00]); // SYN
    frame.extend_from_slice(&[0, 0]);
    frame.extend_from_slice(&[0, 0]); // urgent pointer (TCP header = 20)
    write_u32(&mut out, frame.len() as u32);
    write_u32(&mut out, frame.len() as u32);
    out.extend_from_slice(&frame);
    out
}

fn parse_bytes(data: &[u8]) -> PluginResult<Output> {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "tf-pcap-test-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::write(&path, data).unwrap();
    let out = parse(Args {
        path: path.to_string_lossy().into_owned(),
        limit: None,
    });
    std::fs::remove_file(&path).ok();
    out
}

#[test]
fn parses_both_endiannesses() {
    for big in [true, false] {
        let out = parse_bytes(&build_pcap(big, false)).unwrap();
        assert_eq!(out.big_endian, big);
        assert_eq!(out.link_type, "ethernet");
        assert_eq!(out.total, 1);
        let packet = &out.packets[0];
        assert_eq!(packet.protocol, "TCP");
        assert_eq!(packet.src, "10.0.0.1");
        assert_eq!(packet.dst, "93.184.216.34");
        assert_eq!(packet.src_port, Some(8080));
        assert_eq!(packet.dst_port, Some(80));
        assert_eq!(packet.tcp_flags.as_deref(), Some("SYN"));
        assert_eq!(packet.ts_sec, 1000);
        let conversation = &out.conversations[0];
        assert_eq!(conversation.packets, 1);
        assert_eq!(conversation.bytes, 54);
    }
}

#[test]
fn nanosecond_variant_parses() {
    let out = parse_bytes(&build_pcap(true, true)).unwrap();
    assert!(out.nanosecond);
}

#[test]
fn rejects_bad_magic_and_small_files() {
    let err = parse(Args {
        path: "/nonexistent/demo.pcap".into(),
        limit: None,
    })
    .unwrap_err();
    assert!(
        err.code.starts_with("pcap.") || err.code.starts_with("fs."),
        "unexpected {}",
        err.code
    );
    let err = parse_bytes(&[0u8; 10]).unwrap_err();
    assert_eq!(err.code, "pcap.too_small");
    let mut bad = build_pcap(true, false);
    bad[0] = 0x12;
    let err = parse_bytes(&bad).unwrap_err();
    assert_eq!(err.code, "pcap.bad_magic");
}

#[test]
fn dispatches_via_plugin() {
    assert_eq!(
        PcapViewer::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&PcapViewer::default()).unwrap();
    assert_eq!(checked, 1);
}
