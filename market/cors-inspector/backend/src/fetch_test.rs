use super::*;
use tf_plugin_api::PluginError;

fn headers(list: &[(&str, &str)]) -> Vec<HeaderArg> {
    list.iter()
        .map(|(name, value)| HeaderArg {
            name: (*name).to_owned(),
            value: (*value).to_owned(),
        })
        .collect()
}

fn build(list: &[(&str, &str)]) -> Result<HeaderList, PluginError> {
    HeaderList::build(&headers(list))
}

#[test]
fn safelisted_methods_are_case_sensitive() {
    assert!(is_safelisted_method("GET"));
    assert!(is_safelisted_method("HEAD"));
    assert!(is_safelisted_method("POST"));
    // 方法比较大小写敏感，"get" 不在 safelist 内
    assert!(!is_safelisted_method("get"));
    assert!(!is_safelisted_method("DELETE"));
    assert!(!is_safelisted_method("OPTIONS"));
}

#[test]
fn content_type_essence_decides_safelist() {
    assert!(is_safelisted_request_header("content-type", "text/plain"));
    assert!(is_safelisted_request_header(
        "content-type",
        "Text/Plain; charset=utf-8"
    ));
    assert!(is_safelisted_request_header(
        "content-type",
        "application/x-www-form-urlencoded"
    ));
    assert!(is_safelisted_request_header(
        "content-type",
        "multipart/form-data"
    ));
    // JSON 会触发预检：不能只看头名称忽略值
    assert!(!is_safelisted_request_header(
        "content-type",
        "application/json"
    ));
    // 多个值合并后不是合法的单一类型
    assert!(!is_safelisted_request_header(
        "content-type",
        "text/plain, text/plain"
    ));
    assert!(!is_safelisted_request_header("content-type", ""));
}

#[test]
fn range_only_single_byte_ranges() {
    assert!(is_safelisted_request_header("range", "bytes=0-100"));
    assert!(is_safelisted_request_header("range", "bytes=0-"));
    assert!(!is_safelisted_request_header("range", "bytes=-500"));
    assert!(!is_safelisted_request_header("range", "bytes=100-0"));
    assert!(!is_safelisted_request_header("range", "bytes=0-1, 5-9"));
    assert!(!is_safelisted_request_header("range", "0-100"));
}

#[test]
fn unsafe_request_header_bytes_block_safelist() {
    assert!(is_safelisted_request_header("accept", "*/*"));
    // 值里出现 "(" / ":" 等 CORS-unsafe byte
    assert!(!is_safelisted_request_header("accept", "text/plain(ie)"));
    assert!(!is_safelisted_request_header("accept-language", "en:US"));
}

#[test]
fn non_safelisted_names_always_unsafe() {
    assert!(!is_safelisted_request_header("x-token", "abc"));
    assert!(!is_safelisted_request_header("authorization", "Bearer x"));
}

#[test]
fn forbidden_request_names_include_prefixes() {
    assert!(is_forbidden_request_name("cookie"));
    assert!(is_forbidden_request_name("host"));
    assert!(is_forbidden_request_name("origin"));
    assert!(is_forbidden_request_name("sec-fetch-site"));
    assert!(is_forbidden_request_name("proxy-authorization"));
    assert!(!is_forbidden_request_name("x-custom"));
}

#[test]
fn header_names_must_be_tokens() {
    assert!(is_header_name_token("X-Token"));
    assert!(!is_header_name_token("bad name"));
    assert!(!is_header_name_token(""));
    assert!(!is_header_name_token("naïve"));
}

#[test]
fn header_list_normalizes_names_and_keeps_duplicates() {
    let list = build(&[("X-Token", " a "), ("x-token", "b"), ("Accept", "*/*")]).unwrap();
    assert_eq!(list.get("x-token"), vec!["a", "b"]);
    assert_eq!(list.get("X-TOKEN"), Vec::<&str>::new());
    assert_eq!(list.names(), vec!["x-token", "accept"]);
    assert_eq!(list.unsafe_names(), vec!["x-token"]);
    assert!(list.forbidden_names().is_empty());
}

#[test]
fn header_list_reports_forbidden_names() {
    let list = build(&[
        ("Cookie", "a=b"),
        ("Sec-Fetch-Site", "same-origin"),
        ("X-Ok", "1"),
    ])
    .unwrap();
    assert_eq!(list.forbidden_names(), vec!["cookie", "sec-fetch-site"]);
    assert_eq!(
        list.unsafe_names(),
        vec!["cookie", "sec-fetch-site", "x-ok"]
    );
}

#[test]
fn header_list_rejects_invalid_input() {
    let err = build(&[("bad name", "x")]).unwrap_err();
    assert_eq!(err.code, "cors.invalid_header_name");
    let many: Vec<HeaderArg> = (0..(MAX_HEADERS + 1))
        .map(|i| HeaderArg {
            name: format!("x-{i}"),
            value: String::new(),
        })
        .collect();
    let err = HeaderList::build(&many).unwrap_err();
    assert_eq!(err.code, "cors.header_too_many");
    let err = build(&[("x-big", &"a".repeat(MAX_VALUE + 1))]).unwrap_err();
    assert_eq!(err.code, "cors.header_too_large");
}

#[test]
fn sensitive_values_are_masked_in_views() {
    let view = HeaderView::new("Authorization", "Bearer secret");
    assert!(view.masked);
    assert!(!view.value.contains("secret"));
    let view = HeaderView::new("Accept", "*/*");
    assert!(!view.masked);
    assert_eq!(view.value, "*/*");
}

#[test]
fn response_status_must_be_valid_http() {
    assert!(validate_status(None, "cors.invalid_status").is_ok());
    assert!(validate_status(Some(200), "cors.invalid_status").is_ok());
    assert!(validate_status(Some(599), "cors.invalid_status").is_ok());
    assert_eq!(
        validate_status(Some(99), "cors.invalid_status")
            .unwrap_err()
            .code,
        "cors.invalid_status"
    );
    assert_eq!(
        validate_status(Some(1000), "cors.invalid_status")
            .unwrap_err()
            .code,
        "cors.invalid_status"
    );
}
