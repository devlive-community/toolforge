use super::*;
use crate::parse::parse;
use crate::tokenize::tokenize;

fn req(command: &str) -> Request {
    parse(&tokenize(command).unwrap()).unwrap().0
}

const JSON_POST: &str = "curl -X POST https://api.example.com/users -H 'Authorization: Bearer t' -H 'Content-Type: application/json' -d '{\"name\":\"Ada\",\"tags\":[\"a\"],\"admin\":false,\"boss\":null}'";

#[test]
fn javascript_uses_fetch() {
    let code = javascript(&req(JSON_POST));
    assert!(code.contains("const response = await fetch(\"https://api.example.com/users\", {"));
    assert!(code.contains("  method: \"POST\","));
    assert!(
        code.contains("  body: JSON.stringify({\n    \"name\": \"Ada\","),
        "{code}"
    );
    assert_eq!(
        javascript(&req("curl https://a.b")).lines().next(),
        Some("const response = await fetch(\"https://a.b\");")
    );
    let dup = javascript(&req("curl https://a.b -H 'A: 1' -H 'a: 2'"));
    assert!(dup.contains("  headers: [\n    [\"A\", \"1\"],"), "{dup}");
}

#[test]
fn python_converts_json_to_literals() {
    let code = python(&req(JSON_POST));
    assert!(code.contains("json_data = {\n    \"name\": \"Ada\",\n    \"tags\": [\n        \"a\",\n    ],\n    \"admin\": False,\n    \"boss\": None,\n}"), "{code}");
    assert!(code.contains(
        "response = requests.post(\n    url,\n    headers=headers,\n    json=json_data,\n)"
    ));
    let custom = python(&req("curl -X PURGE https://a.b -k -m 3"));
    assert!(
        custom.contains(
            "requests.request(\n    \"PURGE\",\n    url,\n    verify=False,\n    timeout=3,\n)"
        ),
        "{custom}"
    );
    let merged = python(&req("curl https://a.b -H 'Cookie: a=1' -b 'b=2'"));
    assert!(merged.contains("\"Cookie\": \"a=1; b=2\""), "{merged}");
}

#[test]
fn go_aligns_fields_and_imports() {
    let code = go(&req(
        "curl -k -m 1.5 -x http://p:1 https://a.b -H 'Host: h' -H 'X: 1' -H 'X: 2'",
    ));
    assert!(code.contains("import (\n\t\"crypto/tls\"\n\t\"fmt\"\n\t\"io\"\n\t\"net/http\"\n\t\"net/url\"\n\t\"time\"\n)"), "{code}");
    assert!(code.contains("\t\t\tProxy:           http.ProxyURL(proxyURL),\n\t\t\tTLSClientConfig: &tls.Config{InsecureSkipVerify: true},"), "{code}");
    assert!(code.contains("req.Host = \"h\""));
    assert!(code.contains("req.Header.Set(\"X\", \"1\")\n\treq.Header.Add(\"X\", \"2\")"));
}

#[test]
fn rust_escapes_strings() {
    assert_eq!(rust_string("a\"b\\c\u{1}"), "\"a\\\"b\\\\c\\u{1}\"");
    assert_eq!(rust_string("a\nb"), "r#\"a\nb\"#");
    let code = rust(&req("curl -X PURGE https://a.b"));
    assert!(code.contains(".request(reqwest::Method::from_bytes(b\"PURGE\")?, \"https://a.b\")"));
    assert!(code.contains("let client = reqwest::blocking::Client::new();"));
}

#[test]
fn java_skips_restricted_headers_and_parses_proxy() {
    let code = java(&req(
        "curl -L -x http://user@proxy.local:3128 https://a.b -H 'Host: x' -d a=1",
    ));
    assert!(code.contains("// header \"Host\" is set by java.net.http"));
    assert!(!code.contains(".header(\"Host\""));
    assert!(code.contains("new InetSocketAddress(\"proxy.local\", 3128)"));
    assert!(code.contains(".followRedirects(HttpClient.Redirect.NORMAL)"));
    assert_eq!(host_port("socks5://h:1080/"), Some(("h".into(), 1080)));
    assert_eq!(host_port("h"), None);
}

#[test]
fn php_and_csharp_handle_bodies() {
    let php_code = php(&req(
        "curl https://a.b -F 'f=@/x/a.png;type=image/png' -F n=1",
    ));
    assert!(
        php_code.contains("'f' => new CURLFile('/x/a.png', 'image/png'),"),
        "{php_code}"
    );
    assert!(php(&req("curl https://a.b -d 'it'\"'\"'s'")).contains("'it\\'s'"));
    let cs = csharp(&req(JSON_POST));
    assert!(
        cs.contains("new StringContent(\"\"\"\n{\n  \"name\": \"Ada\","),
        "{cs}"
    );
    assert!(cs.contains("request.Content.Headers.Remove(\"Content-Type\");"));
    let get = csharp(&req("curl https://a.b -H 'Content-Type: text/plain'"));
    assert!(get.contains("// header \"Content-Type\" needs a request body"));
}

#[test]
fn java_builds_multipart_bodies() {
    let code = java(&req(
        "curl https://a.b -F n=1 -F 'f=@/x/a.png;type=image/png'",
    ));
    assert!(code.contains("import java.util.List;"));
    assert!(code.contains(r#"form.add(("--" + boundary + "\r\nContent-Disposition: form-data; name=\"n\"\r\n\r\n1\r\n").getBytes(StandardCharsets.UTF_8));"#), "{code}");
    assert!(code.contains(r#"filename=\"a.png\"\r\nContent-Type: image/png"#));
    assert!(code.contains(".method(\"POST\", HttpRequest.BodyPublishers.ofByteArrays(form))"));
    assert!(
        code.contains(r#".header("Content-Type", "multipart/form-data; boundary=" + boundary)"#)
    );
}
