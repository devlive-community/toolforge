use super::*;
use serde_json::json;

/// 构造一个最小单文件 torrent（name=hello.txt, length=11, piece length=16384）
fn build_torrent() -> Vec<u8> {
    let mut out = String::from("d");
    out.push_str("8:announce");
    out.push_str("31:http://tracker.example/announce");
    out.push_str("4:infod");
    out.push_str("6:lengthi11e");
    out.push_str("4:name9:hello.txt");
    out.push_str("12:piece lengthi16384e");
    out.push_str("6:pieces20:01234567890123456789");
    out.push('e');
    out.push('e');
    out.into_bytes()
}

#[test]
fn sha1_known_vectors() {
    assert_eq!(
        hex(&sha1(b"abc")),
        "a9993e364706816aba3e25717850c26c9cd0d89d"
    );
    assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    assert_eq!(
        hex(&sha1(b"foobar")),
        "8843d7f92416211de9ebb963ff4ce28125932878"
    );
}

#[test]
fn bencode_roundtrip() {
    let (value, consumed) = bencode_parse(b"i42e").unwrap();
    assert_eq!(consumed, 4);
    assert!(matches!(value, Bencode::Int(42)));
    let (value, _) = bencode_parse(b"3:abc").unwrap();
    assert_eq!(bytes_text(&value).as_deref(), Some("abc"));
    let (value, _) = bencode_parse(b"li1ei2ee").unwrap();
    assert!(matches!(value, Bencode::List(list) if list.len() == 2));
    let err = bencode_parse(b"i42").unwrap_err();
    assert_eq!(err.code, "torrent.bad_bencode");
}

#[test]
fn parses_torrent_metadata() {
    let path = std::env::temp_dir().join(format!("tf-torrent-{}.torrent", std::process::id()));
    std::fs::write(&path, build_torrent()).unwrap();
    let out = analyze(Args {
        source: path.to_string_lossy().into_owned(),
    })
    .unwrap();
    assert_eq!(out.kind, "torrent");
    assert_eq!(out.display_name, "hello.txt");
    assert_eq!(out.piece_length, 16384);
    assert_eq!(out.piece_count, 1);
    assert_eq!(out.total_size, 11);
    assert_eq!(out.files[0].path, "hello.txt");
    assert_eq!(out.files[0].size, 11);
    assert_eq!(
        out.trackers,
        vec!["http://tracker.example/announce".to_string()]
    );
    assert_eq!(out.info_hash.len(), 40);
    std::fs::remove_file(&path).ok();
}

#[test]
fn parses_magnet_uri() {
    let out = analyze(Args {
        source: "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=My%20File&tr=http%3A%2F%2Ft.example%2Fannounce".into(),
    })
    .unwrap();
    assert_eq!(out.kind, "magnet");
    assert_eq!(out.info_hash, "0123456789abcdef0123456789abcdef01234567");
    assert_eq!(out.display_name, "My File");
    assert_eq!(out.trackers, vec!["http://t.example/announce".to_string()]);
    let err = analyze(Args {
        source: "magnet:?dn=x".into(),
    })
    .unwrap_err();
    assert_eq!(err.code, "torrent.magnet_no_hash");
}

#[test]
fn validates_sources() {
    let err = analyze(Args {
        source: String::new(),
    })
    .unwrap_err();
    assert_eq!(err.code, "torrent.empty_source");
    let err = analyze(Args {
        source: "/nonexistent/a.torrent".into(),
    })
    .unwrap_err();
    assert!(matches!(
        err.code.as_str(),
        "torrent.read_failed" | "fs.not_found"
    ));
}

#[test]
fn dispatches_via_plugin() {
    assert_eq!(
        TorrentAnalyzer::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&TorrentAnalyzer::default()).unwrap();
    assert_eq!(checked, 1);
}
