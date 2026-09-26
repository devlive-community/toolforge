use super::*;

#[test]
fn builds_requests_with_headers_and_protocols() {
    let args = Args {
        session: "s".into(),
        url: " ws://example.com:8080/chat?room=1 ".into(),
        headers: vec![
            ("Authorization".into(), "Bearer x".into()),
            (" ".into(), "ignored".into()),
        ],
        protocols: vec!["graphql-ws".into(), " chat ".into()],
        timeout_ms: 1000,
    };
    let request = build_request(&args).unwrap();
    assert_eq!(
        request.uri().to_string(),
        "ws://example.com:8080/chat?room=1"
    );
    assert_eq!(request.headers()["authorization"], "Bearer x");
    assert_eq!(
        request.headers()["sec-websocket-protocol"],
        "graphql-ws, chat"
    );

    let bad = |url: &str| {
        build_request(&Args {
            url: url.into(),
            ..args.clone()
        })
        .unwrap_err()
        .code
    };
    assert_eq!(bad("not a url"), "ws.invalid_url");
    assert_eq!(bad("http://example.com"), "ws.invalid_scheme");
    let header = build_request(&Args {
        headers: vec![("bad header".into(), "x".into())],
        ..args.clone()
    });
    assert_eq!(header.unwrap_err().code, "ws.invalid_header");
}
