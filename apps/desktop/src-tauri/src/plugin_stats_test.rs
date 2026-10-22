use super::*;
use std::io::Write;
use std::net::TcpListener;

fn report(now: i64) -> DownloadReport {
    DownloadReport {
        attempt_id: "00000000000000000000000000000001".into(),
        receipt: "test-only-token".into(),
        sha256: "a".repeat(64),
        size_bytes: 42,
        expires_at: now + 1_000_000,
        created_at: now,
        next_attempt_at: now,
        attempts: 0,
    }
}

fn server(response: &'static str) -> (String, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        loop {
            let mut b = [0];
            stream.read_exact(&mut b).unwrap();
            request.push(b[0]);
            if request.ends_with(b"\r\n\r\n") {
                break;
            }
        }
        let headers = String::from_utf8(request.clone())
            .unwrap()
            .to_ascii_lowercase();
        let len: usize = headers
            .lines()
            .find_map(|line| line.strip_prefix("content-length: "))
            .unwrap()
            .parse()
            .unwrap();
        let mut body = vec![0; len];
        stream.read_exact(&mut body).unwrap();
        request.extend(body);
        stream.write_all(response.as_bytes()).unwrap();
        String::from_utf8(request).unwrap()
    });
    (url, thread)
}

#[test]
fn confirmation_success_and_permanent_errors_leave_no_pending_record() {
    for response in [
        "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n",
        "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n",
        "HTTP/1.1 302 Found\r\nLocation: https://evil.test/\r\nContent-Length: 0\r\n\r\n",
    ] {
        let now = now_millis();
        let store = Store::open_in_memory().unwrap();
        store.download_report_enqueue(&report(now)).unwrap();
        let (base, thread) = server(response);
        flush(&client(Duration::from_secs(5)).unwrap(), &store, &base, now).unwrap();
        let request = thread.join().unwrap();
        assert!(request.contains("/v1/downloads/00000000000000000000000000000001/complete"));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer test-only-token")
        );
        assert!(request.contains("\"size\":42"));
        assert!(
            store
                .download_reports_due(now + 100, 10)
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn temporary_failure_is_retried_after_server_deadline() {
    let now = now_millis();
    let store = Store::open_in_memory().unwrap();
    store.download_report_enqueue(&report(now)).unwrap();
    let (base, thread) =
        server("HTTP/1.1 429 Too Many Requests\r\nRetry-After: 120\r\nContent-Length: 0\r\n\r\n");
    flush(&client(Duration::from_secs(5)).unwrap(), &store, &base, now).unwrap();
    thread.join().unwrap();
    assert!(
        store
            .download_reports_due(now + 119_000, 10)
            .unwrap()
            .is_empty()
    );
    let reports = store.download_reports_due(now + 121_000, 10).unwrap();
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].attempts, 1);
}

#[test]
fn retry_after_supports_http_dates_and_never_shortens_backoff() {
    let now = 1_000_000_000;
    let date = httpdate::fmt_http_date(UNIX_EPOCH + Duration::from_millis((now + 120_000) as u64));
    let value = reqwest::header::HeaderValue::from_str(&date).unwrap();
    assert_eq!(retry_after(Some(&value), now), Some(now + 120_000));
    assert!(next_retry(&report(now), now, Some(now + 120_000)) >= now + 120_000);
}
