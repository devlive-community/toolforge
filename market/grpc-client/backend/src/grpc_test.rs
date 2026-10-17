use super::*;

fn entry(key: &str, value: &str) -> MetadataEntry {
    MetadataEntry {
        key: key.into(),
        value: value.into(),
        enabled: true,
        secret: false,
    }
}

#[test]
fn frames_round_trip_across_chunks() {
    let mut decoder = Decoder::default();
    let mut bytes = frame(b"hello");
    bytes.extend(frame(b""));
    bytes.extend(frame(&[7; 300]));
    for chunk in bytes.chunks(3) {
        decoder.push(chunk);
    }
    assert_eq!(decoder.next().unwrap(), Some(b"hello".to_vec()));
    assert_eq!(decoder.next().unwrap(), Some(Vec::new()));
    assert_eq!(decoder.next().unwrap(), Some(vec![7; 300]));
    assert_eq!(decoder.next().unwrap(), None);
    assert!(!decoder.has_partial());
    decoder.push(&[0, 0, 0]);
    assert_eq!(decoder.next().unwrap(), None);
    assert!(decoder.has_partial());

    let mut compressed = Decoder::default();
    compressed.push(&[1, 0, 0, 0, 1, 9]);
    assert_eq!(compressed.next().unwrap_err().code, "grpc.compressed");
    let mut huge = Decoder::default();
    huge.push(&[0, 0xff, 0xff, 0xff, 0xff]);
    assert_eq!(huge.next().unwrap_err().code, "grpc.message_too_large");
}

#[test]
fn reads_status_and_percent_encoded_messages() {
    assert_eq!(decode_message("bad%20input%20%E4%BD%A0"), "bad input 你");
    assert_eq!(decode_message("100%"), "100%");
    assert_eq!(decode_message("%zz"), "%zz");
    let status = status_from(&[
        ("grpc-status".into(), "5".into()),
        ("grpc-message".into(), "no%20item".into()),
    ])
    .unwrap();
    assert_eq!(
        (
            status.code,
            status.name,
            status.message.as_str(),
            status.local
        ),
        (5, "NOT_FOUND", "no item", false)
    );
    assert!(status_from(&[("x".into(), "1".into())]).is_none());
    assert_eq!(status_name(99), "UNKNOWN");
    assert_eq!(http_status_code(404), 12);
    let shown = visible(&[
        ("grpc-status".into(), "0".into()),
        ("content-type".into(), "application/grpc".into()),
        ("x-id".into(), "1".into()),
    ]);
    assert_eq!(shown, [("x-id".to_owned(), "1".to_owned())]);
}

#[test]
fn validates_metadata() {
    let headers = metadata_headers(&[
        entry("Authorization", "Bearer x"),
        entry("trace-bin", "AQID"),
        MetadataEntry {
            enabled: false,
            ..entry("skipped", "x")
        },
        entry(" ", "ignored"),
    ])
    .unwrap();
    assert_eq!(
        headers,
        [
            ("authorization".to_owned(), "Bearer x".to_owned()),
            ("trace-bin".to_owned(), "AQID".to_owned())
        ]
    );
    let code = |e: MetadataEntry| metadata_headers(&[e]).unwrap_err().code;
    assert_eq!(code(entry("grpc-timeout", "1S")), "grpc.metadata_reserved");
    assert_eq!(code(entry("content-type", "x")), "grpc.metadata_reserved");
    assert_eq!(code(entry("bad key", "x")), "grpc.metadata_key");
    assert_eq!(code(entry("x-bin", "***")), "grpc.metadata_binary");
    assert_eq!(code(entry("x", "line\nbreak")), "grpc.metadata_value");
}

#[test]
fn resolves_targets() {
    let target = |address: &str, tls: bool| Target {
        address: address.into(),
        tls,
    };
    assert_eq!(
        target("localhost:50051", false).base().unwrap(),
        "http://localhost:50051"
    );
    assert_eq!(
        target("api.example.com:443/", true).base().unwrap(),
        "https://api.example.com:443"
    );
    assert_eq!(target("https://x:1", false).base().unwrap(), "https://x:1");
    assert_eq!(
        target(" ", false).base().unwrap_err().code,
        "grpc.empty_target"
    );
    assert_eq!(
        target("ftp://x", false).base().unwrap_err().code,
        "grpc.invalid_target"
    );
}
