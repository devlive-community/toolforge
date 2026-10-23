use super::*;
use crate::tokenize::tokenize;

fn p(input: &str) -> (Request, Vec<Warning>) {
    parse(&tokenize(input).unwrap()).unwrap()
}

fn codes(warnings: &[Warning]) -> Vec<&str> {
    warnings.iter().map(|w| w.code.as_str()).collect()
}

#[test]
fn simple_get() {
    let (req, warnings) = p("curl example.com/x");
    assert_eq!(
        (req.method.as_str(), req.url.as_str()),
        ("GET", "http://example.com/x")
    );
    assert_eq!(req.body, Body::None);
    assert!(req.headers.is_empty() && warnings.is_empty());
}

#[test]
fn data_implies_post_and_form_type() {
    let (req, _) = p("curl -sSL https://a.b -d a=1 --data 'b=2\n' -H 'X-Y: z'");
    assert_eq!(req.method, "POST");
    assert!(req.follow_redirects);
    assert_eq!(
        req.body,
        Body::Text {
            text: "a=1&b=2".into()
        }
    );
    assert_eq!(
        req.header("content-type"),
        Some("application/x-www-form-urlencoded")
    );
    assert_eq!(req.headers[0], ("X-Y".into(), "z".into()));
}

#[test]
fn json_and_explicit_method() {
    let (req, _) = p("curl -XPUT https://a.b --json '{\"a\":1}'");
    assert_eq!(req.method, "PUT");
    assert_eq!(req.header("Content-Type"), Some("application/json"));
    assert_eq!(req.header("Accept"), Some("application/json"));
    // 已有的 Content-Type 不会被表单默认值覆盖
    assert_eq!(req.headers.len(), 2);
}

#[test]
fn get_moves_data_to_query_and_head() {
    let (req, _) =
        p("curl -G 'https://a.b/s?x=1' --data-urlencode 'q=hello world' --data-urlencode '=a&b'");
    assert_eq!(req.url, "https://a.b/s?x=1&q=hello+world&a%26b");
    assert_eq!((req.method.as_str(), &req.body), ("GET", &Body::None));
    assert_eq!(p("curl -I https://a.b").0.method, "HEAD");
}

#[test]
fn auth_cookies_and_misc_headers() {
    let (req, warnings) = p(
        "curl -u user:pa:ss -b 'a=1; b=2' -A ua -e https://r --oauth2-bearer tok -k -m 2.5 -x http://p:8080 --compressed https://a.b",
    );
    assert!(
        req.headers
            .iter()
            .any(|(k, v)| k == "Authorization" && v == "Basic dXNlcjpwYTpzcw==")
    );
    assert_eq!(req.header("Cookie"), Some("a=1; b=2"));
    assert_eq!(req.header("User-Agent"), Some("ua"));
    assert_eq!(req.header("Referer"), Some("https://r"));
    assert!(
        req.headers
            .iter()
            .any(|(k, v)| k == "Authorization" && v == "Bearer tok")
    );
    assert!(req.insecure && req.compressed);
    assert_eq!(
        (req.timeout, req.proxy.as_deref()),
        (Some(2.5), Some("http://p:8080"))
    );
    assert!(warnings.is_empty());
}

#[test]
fn multipart_and_file_bodies() {
    let (req, _) = p(
        "curl https://a.b -F name=Ada -F 'avatar=@/tmp/a.png;type=image/png;filename=me.png' --form-string 'raw=@not-a-file'",
    );
    let Body::Multipart { parts } = &req.body else {
        panic!("{:?}", req.body)
    };
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0].value.as_deref(), Some("Ada"));
    assert_eq!(parts[1].file.as_deref(), Some("/tmp/a.png"));
    assert_eq!(parts[1].content_type.as_deref(), Some("image/png"));
    assert_eq!(parts[1].filename.as_deref(), Some("me.png"));
    assert_eq!(parts[2].value.as_deref(), Some("@not-a-file"));
    assert_eq!(req.method, "POST");
    assert_eq!(req.header("Content-Type"), None);

    let (req, _) =
        p("curl https://a.b --data-binary @body.json -H 'content-type: application/json'");
    assert_eq!(
        req.body,
        Body::File {
            path: "body.json".into()
        }
    );
    assert_eq!(req.headers.len(), 1);
}

#[test]
fn warnings_for_unsupported_input() {
    let (req, warnings) =
        p("curl --frobnicate -T up.bin -b jar.txt https://a.b https://c.d -o out.txt --retry 3");
    assert_eq!(req.url, "https://a.b");
    assert_eq!(
        codes(&warnings),
        vec![
            "curl.unknown_option",
            "curl.upload_unsupported",
            "curl.cookie_file",
            "curl.multiple_urls"
        ]
    );
    assert_eq!(warnings[0].params["option"], "--frobnicate");
}

#[test]
fn errors() {
    let err = |input: &str| parse(&tokenize(input).unwrap()).unwrap_err().code;
    assert_eq!(err("curl -H"), "curl.missing_value");
    assert_eq!(err("curl -s"), "curl.no_url");
    assert_eq!(err(""), "curl.no_url");
}
