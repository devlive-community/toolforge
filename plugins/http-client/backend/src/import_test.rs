use super::*;

#[test]
fn imports_browser_curl_with_query_headers_json_and_settings() {
    let imported = import(r#"curl 'https://example.com/users?q=a%20b' -X PATCH -H 'X-Token: abc' --json '{"name":"Ada"}' -L --max-time 12.5"#).unwrap();
    let request = imported.request;
    assert_eq!(request.method, "PATCH");
    assert_eq!(request.url, "https://example.com/users?q=a%20b");
    assert_eq!(request.body.kind, BodyKind::Json);
    assert_eq!(request.body.text, r#"{"name":"Ada"}"#);
    assert_eq!(request.timeout_ms, 12_500);
    assert!(request.follow_redirects);
    assert!(request.use_proxy);
    assert!(request.params.is_empty());
    assert!(
        request
            .headers
            .iter()
            .any(|header| header.key == "X-Token" && header.value == "abc")
    );
    assert!(imported.warnings.is_empty());
}

#[test]
fn preserves_encoded_form_data_and_does_not_execute_shell_text() {
    let imported = import("curl https://example.com -d 'a=hello+world&b=%26' -H 'X-Exec: $(touch /tmp/never-run)' --proxy ''").unwrap();
    assert_eq!(imported.request.body.kind, BodyKind::Text);
    assert_eq!(imported.request.body.text, "a=hello+world&b=%26");
    assert_eq!(imported.request.headers[0].value, "$(touch /tmp/never-run)");
    assert_eq!(imported.request.timeout_ms, 30_000);
    assert!(!imported.request.follow_redirects);
    assert!(!imported.request.use_proxy);
}

#[test]
fn warns_about_settings_that_cannot_be_imported_equivalently() {
    let imported = import("curl https://example.com -k --compressed -x http://localhost:9000 --max-time 0.1 --unknown").unwrap();
    let codes: Vec<_> = imported
        .warnings
        .iter()
        .map(|warning| warning.code.as_str())
        .collect();
    assert_eq!(
        codes,
        [
            "curl.unknown_option",
            "http.curl_insecure",
            "http.curl_proxy",
            "http.curl_compressed",
            "http.curl_timeout_adjusted"
        ]
    );
    assert_eq!(imported.request.timeout_ms, 1000);
}

#[test]
fn rejects_file_uploads_multipart_and_methods_without_losing_data() {
    for command in [
        "curl https://example.com -d @/definitely/not/read",
        "curl https://example.com -F name=value",
        "curl https://example.com -F file=@/definitely/not/read",
    ] {
        assert_eq!(
            import(command).unwrap_err().code,
            "http.curl_body_unsupported"
        );
    }
    for command in [
        "curl https://example.com -T /definitely/not/read",
        "curl https://example.com -d a=1 -d @/definitely/not/read",
        "curl https://example.com --data-urlencode name@/definitely/not/read",
        "curl https://example.com -b /definitely/not/read",
    ] {
        assert_eq!(
            import(command).unwrap_err().code,
            "http.curl_file_unsupported"
        );
    }
    assert_eq!(
        import("curl https://example.com -X TRACE")
            .unwrap_err()
            .code,
        "http.curl_method_unsupported"
    );
    assert_eq!(
        import("curl ftp://example.com").unwrap_err().code,
        "http.unsupported_scheme"
    );
}

#[test]
fn enforces_curl_and_request_budgets() {
    assert_eq!(
        import(&"x".repeat(1024 * 1024 + 1)).unwrap_err().code,
        "curl.too_large"
    );
    let command = format!(
        "curl https://example.com --data-raw '{}'",
        "x".repeat(512 * 1024 + 1)
    );
    assert_eq!(import(&command).unwrap_err().code, "http.limit_exceeded");
    assert_eq!(
        import("curl 'unfinished").unwrap_err().code,
        "curl.unterminated_quote"
    );
}

#[test]
fn warns_for_settings_ignored_by_the_shared_curl_model() {
    let imported =
        import("curl https://example.com --connect-timeout 2 --http1.1 --cacert /not/read -sS -0")
            .unwrap();
    let options: Vec<_> = imported
        .warnings
        .iter()
        .map(|warning| warning.params["option"].as_str().unwrap())
        .collect();
    assert_eq!(
        options,
        ["-0", "--connect-timeout", "--http1.1", "--cacert"]
    );
}
