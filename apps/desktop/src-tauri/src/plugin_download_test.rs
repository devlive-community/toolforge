use super::*;
use std::collections::BTreeMap;
use std::io::Write;
use std::net::TcpListener;

fn expected() -> Expected<'static> {
    Expected {
        id: "org.example.demo",
        version: "1.0.0",
        sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        size: 42,
    }
}

fn package() -> Vec<u8> {
    tf_package::build(&BTreeMap::from([
        ("manifest.json".to_owned(), br#"{"id":"org.example.demo","version":"1.0.0","name":"Demo","description":"d","category":"other","functions":{"run":{}}}"#.to_vec()),
        ("plugin.wasm".to_owned(), b"\0asm".to_vec()),
        ("ui/index.js".to_owned(), b"export default () => null".to_vec()),
    ]), None).unwrap()
}

fn serve_once(response: String) -> (String, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        loop {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
            if request.ends_with(b"\r\n\r\n") {
                break;
            }
        }
        stream.write_all(response.as_bytes()).unwrap();
        String::from_utf8(request).unwrap()
    });
    (url, thread)
}

#[test]
fn official_origin_path_and_asset_hosts_are_strict() {
    let expected = expected();
    let correct = format!(
        "{STATS_BASE}/v1/plugins/{}/versions/{}/download",
        expected.id, expected.version
    );
    assert!(validate_api(&Url::parse(&correct).unwrap(), &expected).is_ok());
    for url in [
        correct.replace("https://", "http://"),
        correct.replace(
            "stats.toolforge.devlive.org",
            "stats.toolforge.devlive.org.evil.test",
        ),
        format!("{correct}?redirect=https://evil.test"),
        format!("{correct}#receipt"),
        correct.replace("/1.0.0/", "/2.0.0/"),
        correct.replace("stats.toolforge", "user:password@stats.toolforge"),
    ] {
        assert!(
            validate_api(&Url::parse(&url).unwrap(), &expected).is_err(),
            "{url}"
        );
    }
    assert!(allowed_asset(
        &Url::parse("https://plugins.toolforge.devlive.org/packages/a.tfp").unwrap()
    ));
    for url in [
        "https://evil.test/a.tfp",
        "http://plugins.toolforge.devlive.org/a.tfp",
        "https://plugins.toolforge.devlive.org:444/a.tfp",
        "https://x@plugins.toolforge.devlive.org/a.tfp",
    ] {
        assert!(!allowed_asset(&Url::parse(url).unwrap()), "{url}");
    }
}

#[test]
fn package_metadata_checksum_and_official_trust_are_required() {
    let bytes = package();
    use sha2::Digest;
    let sha = resources::to_hex(&sha2::Sha256::digest(&bytes));
    let mut expected = Expected {
        sha256: &sha,
        size: bytes.len() as u64,
        ..expected()
    };
    assert!(check_bytes(&bytes, &expected, false).is_ok());
    assert_eq!(
        check_bytes(&bytes, &expected, true).unwrap_err().code,
        "market.unsigned"
    );
    expected.version = "2.0.0";
    assert_eq!(
        check_bytes(&bytes, &expected, false).unwrap_err().code,
        "market.download_metadata_mismatch"
    );
    expected.version = "1.0.0";
    expected.id = "org.example.different";
    assert_eq!(
        check_bytes(&bytes, &expected, false).unwrap_err().code,
        "market.download_metadata_mismatch"
    );
    expected.id = "org.example.demo";
    expected.size += 1;
    assert_eq!(
        check_bytes(&bytes, &expected, false).unwrap_err().code,
        "market.download_metadata_mismatch"
    );
    expected.size -= 1;
    expected.sha256 = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    assert_eq!(
        check_bytes(&bytes, &expected, false).unwrap_err().code,
        "package.checksum_mismatch"
    );
}

#[test]
fn receipt_wrong_attempt_or_foreign_redirect_never_enters_queue() {
    let store = Store::open_in_memory().unwrap();
    let attempt = "00000000000000000000000000000001";
    for returned_attempt in [attempt, "00000000000000000000000000000002"] {
        let response = format!(
            "HTTP/1.1 302 Found\r\nLocation: https://evil.test/a.tfp\r\nX-ToolForge-Attempt-Id: {returned_attempt}\r\nX-ToolForge-Receipt: test-only-token\r\nX-ToolForge-Receipt-Expires: {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            now_millis() / 1000 + 1000
        );
        let (base, server) = serve_once(response);
        assert!(
            fetch_attempt(
                &client(Duration::from_secs(5)).unwrap(),
                &Url::parse(&base).unwrap(),
                &expected(),
                &store,
                attempt
            )
            .is_err()
        );
        let request = server.join().unwrap();
        assert!(
            request
                .to_ascii_lowercase()
                .contains(&format!("idempotency-key: {attempt}"))
        );
        assert!(!request.contains("test-only-token"));
        assert!(
            store
                .download_reports_due(now_millis(), 10)
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn unsigned_asset_is_not_reported_and_receipt_is_never_forwarded() {
    let package = package();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let asset = format!("http://{}/package.tfp", listener.local_addr().unwrap());
    let bytes = package.clone();
    let asset_server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        loop {
            let mut b = [0];
            stream.read_exact(&mut b).unwrap();
            request.push(b[0]);
            if request.ends_with(b"\r\n\r\n") {
                break;
            }
        }
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            bytes.len()
        )
        .unwrap();
        stream.write_all(&bytes).unwrap();
        String::from_utf8(request).unwrap()
    });
    let attempt = "00000000000000000000000000000001";
    let (base, api) = serve_once(format!(
        "HTTP/1.1 302 Found\r\nLocation: {asset}\r\nX-ToolForge-Attempt-Id: {attempt}\r\nX-ToolForge-Receipt: test-only-token\r\nX-ToolForge-Receipt-Expires: {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        now_millis() / 1000 + 1000
    ));
    use sha2::Digest;
    let sha = resources::to_hex(&sha2::Sha256::digest(&package));
    let expected = Expected {
        sha256: &sha,
        size: package.len() as u64,
        ..expected()
    };
    let store = Store::open_in_memory().unwrap();
    assert_eq!(
        fetch_attempt(
            &client(Duration::from_secs(5)).unwrap(),
            &Url::parse(&base).unwrap(),
            &expected,
            &store,
            attempt
        )
        .unwrap_err()
        .code,
        "market.unsigned"
    );
    let request = asset_server.join().unwrap().to_ascii_lowercase();
    assert!(!request.contains("test-only-token"));
    assert!(!request.contains("authorization"));
    assert!(!request.contains("idempotency-key"));
    api.join().unwrap();
    assert!(
        store
            .download_reports_due(now_millis(), 10)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn throttled_download_and_retry_after_are_returned_without_burst_retries() {
    for response in [
        "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 120\r\nContent-Length: 0\r\n\r\n",
        "HTTP/1.1 503 Unavailable\r\nRetry-After: Wed, 21 Oct 2030 07:28:00 GMT\r\nContent-Length: 0\r\n\r\n",
    ] {
        let (base, server) = serve_once(response.to_owned());
        let result = get(
            &client(Duration::from_secs(5)).unwrap(),
            &Url::parse(&base).unwrap(),
            Some("00000000000000000000000000000001"),
        )
        .unwrap();
        assert!(matches!(result.status().as_u16(), 429 | 503));
        server.join().unwrap();
    }
}

#[test]
fn official_trust_cannot_enter_untracked_path_and_identity_stays_bound() {
    let bytes = package();
    let mut package = tf_package::open(&bytes).unwrap();
    package.trust = tf_package::Trust::Official {
        key_id: "synthetic-test-key".into(),
    };
    let expected = expected();
    assert!(check_package(&package, &expected, true).is_ok());
    assert_eq!(
        check_package(&package, &expected, false).unwrap_err().code,
        "market.invalid_download"
    );
    package.manifest.id = "org.example.different".into();
    assert_eq!(
        check_package(&package, &expected, true).unwrap_err().code,
        "market.download_metadata_mismatch"
    );
}

#[test]
fn handshake_network_retries_reuse_the_same_attempt() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
    let server = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for status in ["500 Internal Server Error", "302 Found"] {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            loop {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
                if request.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            requests.push(String::from_utf8(request).unwrap());
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
        }
        requests
    });
    let attempt = "00000000000000000000000000000001";
    let response = get(
        &client(Duration::from_secs(5)).unwrap(),
        &url,
        Some(attempt),
    )
    .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::FOUND);
    for request in server.join().unwrap() {
        assert!(
            request
                .to_ascii_lowercase()
                .contains(&format!("idempotency-key: {attempt}"))
        );
    }
}
