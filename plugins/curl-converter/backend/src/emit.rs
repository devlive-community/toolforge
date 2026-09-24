//! 把请求模型生成为各语言的请求代码。生成的代码中的注释属于代码内容，统一使用英文。

use serde_json::Value;

use crate::parse::{Body, Part, Request};

/// 生成代码时按请求体类型处理：JSON 请求体会格式化为更易读的字面量
fn json_body(req: &Request) -> Option<Value> {
    let Body::Text { text } = &req.body else {
        return None;
    };
    if !req
        .header("Content-Type")?
        .to_ascii_lowercase()
        .contains("json")
    {
        return None;
    }
    serde_json::from_str::<Value>(text)
        .ok()
        .filter(|v| v.is_object() || v.is_array())
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}

/// JSON 字符串字面量，同时是合法的 JavaScript / Python / Go / Java / C# 字符串
fn quote(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_default()
}

fn indent(text: &str, prefix: &str) -> String {
    text.lines()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("{prefix}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn is_multipart(req: &Request) -> bool {
    matches!(req.body, Body::Multipart { .. })
}

/// multipart 的 Content-Type（含 boundary）由各语言的库生成
fn headers(req: &Request) -> Vec<(&str, &str)> {
    req.headers
        .iter()
        .filter(|(k, _)| !(is_multipart(req) && k.eq_ignore_ascii_case("content-type")))
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect()
}

/// 同名请求头合并为一个（用于只能表达映射的语言）
fn merged_headers(req: &Request) -> Vec<(String, String)> {
    let mut merged: Vec<(String, String)> = Vec::new();
    for (name, value) in headers(req) {
        match merged
            .iter_mut()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
        {
            Some((k, v)) => {
                let separator = if k.eq_ignore_ascii_case("cookie") {
                    "; "
                } else {
                    ", "
                };
                v.push_str(separator);
                v.push_str(value);
            }
            None => merged.push((name.to_owned(), value.to_owned())),
        }
    }
    merged
}

fn parts(req: &Request) -> &[Part] {
    match &req.body {
        Body::Multipart { parts } => parts,
        _ => &[],
    }
}

fn millis(seconds: f64) -> u64 {
    (seconds * 1000.0).round() as u64
}

pub fn javascript(req: &Request) -> String {
    let mut out = Vec::new();
    let reads_file =
        matches!(req.body, Body::File { .. }) || parts(req).iter().any(|p| p.file.is_some());
    if reads_file {
        out.push("import { readFile } from \"node:fs/promises\";".to_owned());
        out.push(String::new());
    }
    if req.insecure {
        out.push("// curl -k: fetch cannot skip TLS verification; for local testing only, run with NODE_TLS_REJECT_UNAUTHORIZED=0".into());
    }
    if let Some(proxy) = &req.proxy {
        out.push(format!(
            "// curl -x {proxy}: fetch has no proxy option; use an agent such as undici's ProxyAgent"
        ));
    }
    if is_multipart(req) {
        out.push("const form = new FormData();".into());
        for part in parts(req) {
            match (&part.value, &part.file) {
                (Some(value), _) => out.push(format!(
                    "form.append({}, {});",
                    quote(&part.name),
                    quote(value)
                )),
                (None, Some(file)) => {
                    let options = part
                        .content_type
                        .as_ref()
                        .map(|t| format!(", {{ type: {} }}", quote(t)))
                        .unwrap_or_default();
                    let name = part.filename.as_deref().unwrap_or_else(|| basename(file));
                    out.push(format!(
                        "form.append({}, new Blob([await readFile({})]{options}), {});",
                        quote(&part.name),
                        quote(file),
                        quote(name)
                    ));
                }
                (None, None) => {}
            }
        }
        out.push(String::new());
    }

    let mut options = Vec::new();
    if req.method != "GET" {
        options.push(format!("  method: {},", quote(&req.method)));
    }
    let list = headers(req);
    if !list.is_empty() {
        let duplicated = list.iter().enumerate().any(|(i, (k, _))| {
            list[..i]
                .iter()
                .any(|(other, _)| other.eq_ignore_ascii_case(k))
        });
        if duplicated {
            options.push("  headers: [".into());
            for (k, v) in &list {
                options.push(format!("    [{}, {}],", quote(k), quote(v)));
            }
            options.push("  ],".into());
        } else {
            options.push("  headers: {".into());
            for (k, v) in &list {
                options.push(format!("    {}: {},", quote(k), quote(v)));
            }
            options.push("  },".into());
        }
    }
    match (&req.body, json_body(req)) {
        (_, Some(value)) => options.push(format!(
            "  body: JSON.stringify({}),",
            indent(&pretty(&value), "  ").trim_start()
        )),
        (Body::Text { text }, None) => options.push(format!("  body: {},", quote(text))),
        (Body::File { path }, None) => {
            options.push(format!("  body: await readFile({}),", quote(path)))
        }
        (Body::Multipart { .. }, None) => options.push("  body: form,".into()),
        (Body::None, None) => {}
    }
    if let Some(timeout) = req.timeout {
        options.push(format!(
            "  signal: AbortSignal.timeout({}),",
            millis(timeout)
        ));
    }
    if options.is_empty() {
        out.push(format!(
            "const response = await fetch({});",
            quote(&req.url)
        ));
    } else {
        out.push(format!(
            "const response = await fetch({}, {{",
            quote(&req.url)
        ));
        out.extend(options);
        out.push("});".into());
    }
    out.push(String::new());
    out.push("console.log(response.status);".into());
    out.push("console.log(await response.text());".into());
    out.join("\n") + "\n"
}

fn python_literal(value: &Value, level: usize) -> String {
    let pad = "    ".repeat(level + 1);
    let close = "    ".repeat(level);
    match value {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => quote(s),
        Value::Array(items) if items.is_empty() => "[]".into(),
        Value::Array(items) => {
            let body: Vec<String> = items
                .iter()
                .map(|item| format!("{pad}{},", python_literal(item, level + 1)))
                .collect();
            format!("[\n{}\n{close}]", body.join("\n"))
        }
        Value::Object(map) if map.is_empty() => "{}".into(),
        Value::Object(map) => {
            let body: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{pad}{}: {},", quote(k), python_literal(v, level + 1)))
                .collect();
            format!("{{\n{}\n{close}}}", body.join("\n"))
        }
    }
}

pub fn python(req: &Request) -> String {
    let mut out = vec!["import requests".to_owned(), String::new()];
    let mut args = vec!["url".to_owned()];
    out.push(format!("url = {}", quote(&req.url)));
    let list = merged_headers(req);
    if !list.is_empty() {
        out.push("headers = {".into());
        for (k, v) in &list {
            out.push(format!("    {}: {},", quote(k), quote(v)));
        }
        out.push("}".into());
        args.push("headers=headers".into());
    }
    match (&req.body, json_body(req)) {
        (_, Some(value)) => {
            out.push(format!("json_data = {}", python_literal(&value, 0)));
            args.push("json=json_data".into());
        }
        (Body::Text { text }, None) => {
            out.push(format!("data = {}", quote(text)));
            args.push("data=data".into());
        }
        (Body::File { path }, None) => {
            out.push(format!("with open({}, \"rb\") as f:", quote(path)));
            out.push("    data = f.read()".into());
            args.push("data=data".into());
        }
        (Body::Multipart { parts }, None) => {
            out.push("files = [".into());
            for part in parts {
                let value = match (&part.value, &part.file) {
                    (Some(value), _) => match &part.content_type {
                        Some(t) => format!("(None, {}, {})", quote(value), quote(t)),
                        None => format!("(None, {})", quote(value)),
                    },
                    (None, Some(file)) => {
                        let name = part.filename.as_deref().unwrap_or_else(|| basename(file));
                        match &part.content_type {
                            Some(t) => format!(
                                "({}, open({}, \"rb\"), {})",
                                quote(name),
                                quote(file),
                                quote(t)
                            ),
                            None => format!("({}, open({}, \"rb\"))", quote(name), quote(file)),
                        }
                    }
                    (None, None) => continue,
                };
                out.push(format!("    ({}, {value}),", quote(&part.name)));
            }
            out.push("]".into());
            args.push("files=files".into());
        }
        (Body::None, None) => {}
    }
    if let Some(proxy) = &req.proxy {
        out.push(format!(
            "proxies = {{\"http\": {0}, \"https\": {0}}}",
            quote(proxy)
        ));
        args.push("proxies=proxies".into());
    }
    if req.insecure {
        args.push("verify=False".into());
    }
    if let Some(timeout) = req.timeout {
        args.push(format!("timeout={timeout}"));
    }
    if req.follow_redirects && req.method == "HEAD" {
        args.push("allow_redirects=True".into());
    }

    let method = req.method.to_ascii_lowercase();
    let call = if matches!(
        method.as_str(),
        "get" | "post" | "put" | "patch" | "delete" | "head" | "options"
    ) {
        format!("requests.{method}(")
    } else {
        args.insert(0, quote(&req.method));
        "requests.request(".to_owned()
    };
    out.push(String::new());
    if args.len() == 1 {
        out.push(format!("response = {call}{})", args[0]));
    } else {
        out.push(format!("response = {call}"));
        for arg in &args {
            out.push(format!("    {arg},"));
        }
        out.push(")".into());
    }
    out.push("print(response.status_code)".into());
    out.push("print(response.text)".into());
    out.join("\n") + "\n"
}

/// Go 字符串：多行且不含反引号时用原始字符串
fn go_string(text: &str) -> String {
    if text.contains('\n') && !text.contains('`') {
        format!("`{text}`")
    } else {
        quote(text)
    }
}

/// gofmt 会对齐连续的单行键值对
fn go_fields(fields: &[(&str, String)], prefix: &str) -> Vec<String> {
    let width = fields.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    fields
        .iter()
        .map(|(key, value)| format!("{prefix}{key}:{} {value},", " ".repeat(width - key.len())))
        .collect()
}

const GO_CHECK: [&str; 3] = ["\tif err != nil {", "\t\tpanic(err)", "\t}"];

pub fn go(req: &Request) -> String {
    let mut imports = vec!["fmt", "io", "net/http"];
    let mut body = Vec::new();
    let body_var = match (&req.body, json_body(req)) {
        (_, Some(value)) => {
            imports.push("strings");
            body.push(format!(
                "\tbody := strings.NewReader({})",
                go_string(&pretty(&value))
            ));
            "body"
        }
        (Body::Text { text }, None) => {
            imports.push("strings");
            body.push(format!("\tbody := strings.NewReader({})", go_string(text)));
            "body"
        }
        (Body::File { path }, None) => {
            imports.push("os");
            body.push(format!("\tbody, err := os.Open({})", quote(path)));
            body.extend(GO_CHECK.map(String::from));
            body.push("\tdefer body.Close()".into());
            "body"
        }
        (Body::Multipart { parts }, None) => {
            imports.extend(["bytes", "mime/multipart"]);
            body.push("\tbody := &bytes.Buffer{}".into());
            body.push("\twriter := multipart.NewWriter(body)".into());
            for (i, part) in parts.iter().enumerate() {
                let n = i + 1;
                match (&part.value, &part.file) {
                    (Some(value), _) => {
                        body.push(format!(
                            "\tif err := writer.WriteField({}, {}); err != nil {{",
                            quote(&part.name),
                            quote(value)
                        ));
                        body.push("\t\tpanic(err)".into());
                        body.push("\t}".into());
                    }
                    (None, Some(file)) => {
                        if !imports.contains(&"os") {
                            imports.push("os");
                        }
                        let name = part.filename.as_deref().unwrap_or_else(|| basename(file));
                        body.push(format!("\tfile{n}, err := os.Open({})", quote(file)));
                        body.extend(GO_CHECK.map(String::from));
                        body.push(format!("\tdefer file{n}.Close()"));
                        body.push(format!(
                            "\tpart{n}, err := writer.CreateFormFile({}, {})",
                            quote(&part.name),
                            quote(name)
                        ));
                        body.extend(GO_CHECK.map(String::from));
                        body.push(format!(
                            "\tif _, err := io.Copy(part{n}, file{n}); err != nil {{"
                        ));
                        body.push("\t\tpanic(err)".into());
                        body.push("\t}".into());
                    }
                    (None, None) => {}
                }
            }
            body.push("\tif err := writer.Close(); err != nil {".into());
            body.push("\t\tpanic(err)".into());
            body.push("\t}".into());
            "body"
        }
        (Body::None, None) => "nil",
    };

    let mut main = body;
    if let Some(proxy) = &req.proxy {
        imports.push("net/url");
        main.push(format!("\tproxyURL, err := url.Parse({})", quote(proxy)));
        main.extend(GO_CHECK.map(String::from));
    }
    main.push(format!(
        "\treq, err := http.NewRequest({}, {}, {body_var})",
        quote(&req.method),
        quote(&req.url)
    ));
    main.extend(GO_CHECK.map(String::from));
    let mut seen: Vec<String> = Vec::new();
    for (name, value) in headers(req) {
        if name.eq_ignore_ascii_case("host") {
            main.push(format!("\treq.Host = {}", quote(value)));
            continue;
        }
        let lower = name.to_ascii_lowercase();
        let verb = if seen.contains(&lower) { "Add" } else { "Set" };
        seen.push(lower);
        main.push(format!(
            "\treq.Header.{verb}({}, {})",
            quote(name),
            quote(value)
        ));
    }
    if is_multipart(req) {
        main.push("\treq.Header.Set(\"Content-Type\", writer.FormDataContentType())".into());
    }

    let mut transport = Vec::new();
    if req.proxy.is_some() {
        transport.push(("Proxy", "http.ProxyURL(proxyURL)".to_owned()));
    }
    if req.insecure {
        imports.push("crypto/tls");
        transport.push((
            "TLSClientConfig",
            "&tls.Config{InsecureSkipVerify: true}".to_owned(),
        ));
    }
    let mut client_fields = Vec::new();
    if let Some(timeout) = req.timeout {
        imports.push("time");
        client_fields.push(("Timeout", format!("{} * time.Millisecond", millis(timeout))));
    }
    if client_fields.is_empty() && transport.is_empty() {
        main.push("\tclient := &http.Client{}".into());
    } else {
        main.push("\tclient := &http.Client{".into());
        main.extend(go_fields(&client_fields, "\t\t"));
        if !transport.is_empty() {
            main.push("\t\tTransport: &http.Transport{".into());
            main.extend(go_fields(&transport, "\t\t\t"));
            main.push("\t\t},".into());
        }
        main.push("\t}".into());
    }
    main.push("\tresp, err := client.Do(req)".into());
    main.extend(GO_CHECK.map(String::from));
    main.push("\tdefer resp.Body.Close()".into());
    main.push("\tdata, err := io.ReadAll(resp.Body)".into());
    main.extend(GO_CHECK.map(String::from));
    main.push("\tfmt.Println(resp.Status)".into());
    main.push("\tfmt.Println(string(data))".into());

    imports.sort_unstable();
    imports.dedup();
    let mut out = vec!["package main".to_owned(), String::new(), "import (".into()];
    out.extend(imports.iter().map(|i| format!("\t\"{i}\"")));
    out.push(")".into());
    out.push(String::new());
    out.push("func main() {".into());
    out.extend(main);
    out.push("}".into());
    out.join("\n") + "\n"
}

/// Rust 字符串：多行时用原始字符串
fn rust_string(text: &str) -> String {
    if text.contains('\n') && !text.contains("\"#") {
        return format!("r#\"{text}\"#");
    }
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn rust(req: &Request) -> String {
    let multipart = is_multipart(req);
    let features = if multipart {
        "\"blocking\", \"multipart\""
    } else {
        "\"blocking\""
    };
    let mut out = vec![
        format!("// Cargo.toml: reqwest = {{ version = \"0.13\", features = [{features}] }}"),
        String::new(),
        "fn main() -> Result<(), Box<dyn std::error::Error>> {".to_owned(),
    ];
    let mut builder = Vec::new();
    if req.insecure {
        builder.push("        .danger_accept_invalid_certs(true)".to_owned());
    }
    if let Some(timeout) = req.timeout {
        builder.push(format!(
            "        .timeout(std::time::Duration::from_millis({}))",
            millis(timeout)
        ));
    }
    if let Some(proxy) = &req.proxy {
        builder.push(format!(
            "        .proxy(reqwest::Proxy::all({})?)",
            rust_string(proxy)
        ));
    }
    if builder.is_empty() {
        out.push("    let client = reqwest::blocking::Client::new();".into());
    } else {
        out.push("    let client = reqwest::blocking::Client::builder()".into());
        out.extend(builder);
        out.push("        .build()?;".into());
    }
    if multipart {
        out.push("    let form = reqwest::blocking::multipart::Form::new()".into());
        let last = parts(req).len();
        for (i, part) in parts(req).iter().enumerate() {
            let end = if i + 1 == last { ";" } else { "" };
            let name = rust_string(&part.name);
            match (&part.value, &part.file) {
                (Some(value), _) => out.push(format!(
                    "        .text({name}, {}){end}",
                    rust_string(value)
                )),
                (None, Some(file)) if part.filename.is_none() && part.content_type.is_none() => out
                    .push(format!(
                        "        .file({name}, {})?{end}",
                        rust_string(file)
                    )),
                (None, Some(file)) => {
                    let mut piece = format!(
                        "reqwest::blocking::multipart::Part::file({})?",
                        rust_string(file)
                    );
                    if let Some(filename) = &part.filename {
                        piece.push_str(&format!(".file_name({})", rust_string(filename)));
                    }
                    if let Some(t) = &part.content_type {
                        piece.push_str(&format!(".mime_str({})?", rust_string(t)));
                    }
                    out.push(format!("        .part({name}, {piece}){end}"));
                }
                (None, None) => {}
            }
        }
    }
    let method = req.method.to_ascii_lowercase();
    out.push("    let response = client".into());
    if matches!(
        method.as_str(),
        "get" | "post" | "put" | "patch" | "delete" | "head"
    ) {
        out.push(format!("        .{method}({})", rust_string(&req.url)));
    } else {
        out.push(format!(
            "        .request(reqwest::Method::from_bytes(b{})?, {})",
            rust_string(&req.method),
            rust_string(&req.url)
        ));
    }
    for (name, value) in headers(req) {
        out.push(format!(
            "        .header({}, {})",
            rust_string(name),
            rust_string(value)
        ));
    }
    match (&req.body, json_body(req)) {
        (_, Some(value)) => out.push(format!(
            "        .body({})",
            indent(&rust_string(&pretty(&value)), "        ").trim_start()
        )),
        (Body::Text { text }, None) => out.push(format!("        .body({})", rust_string(text))),
        (Body::File { path }, None) => out.push(format!(
            "        .body(std::fs::read({})?)",
            rust_string(path)
        )),
        (Body::Multipart { .. }, None) => out.push("        .multipart(form)".into()),
        (Body::None, None) => {}
    }
    out.push("        .send()?;".into());
    out.push("    println!(\"{}\", response.status());".into());
    out.push("    println!(\"{}\", response.text()?);".into());
    out.push("    Ok(())".into());
    out.push("}".into());
    out.join("\n") + "\n"
}

/// java.net.http 不允许手动设置的请求头
const JAVA_RESTRICTED: &[&str] = &["connection", "content-length", "expect", "host", "upgrade"];

/// `scheme://host:port` → (host, port)
fn host_port(proxy: &str) -> Option<(String, u16)> {
    let rest = proxy.split_once("://").map_or(proxy, |(_, r)| r);
    let authority = rest.split('/').next()?;
    let authority = authority.rsplit('@').next()?;
    let (host, port) = authority.rsplit_once(':')?;
    Some((host.to_owned(), port.parse().ok()?))
}

/// java.net.http 没有 multipart 支持：按 RFC 7578 手动拼接各部分
fn java_multipart(
    parts: &[Part],
    pad: &str,
    imports: &mut Vec<&'static str>,
    main: &mut Vec<String>,
) {
    imports.extend([
        "java.nio.charset.StandardCharsets",
        "java.util.ArrayList",
        "java.util.List",
    ]);
    main.push(format!(
        r#"{pad}String boundary = "----ToolForge" + System.currentTimeMillis();"#
    ));
    main.push(format!("{pad}List<byte[]> form = new ArrayList<>();"));
    let add = |text: String| {
        format!(
            r#"{pad}form.add(("--" + boundary + {}).getBytes(StandardCharsets.UTF_8));"#,
            quote(&text)
        )
    };
    for part in parts {
        let disposition = format!("\r\nContent-Disposition: form-data; name=\"{}\"", part.name);
        match (&part.value, &part.file) {
            (Some(value), _) => {
                let content_type = part
                    .content_type
                    .as_ref()
                    .map(|t| format!("\r\nContent-Type: {t}"))
                    .unwrap_or_default();
                main.push(add(format!(
                    "{disposition}{content_type}\r\n\r\n{value}\r\n"
                )));
            }
            (None, Some(file)) => {
                imports.extend(["java.nio.file.Files", "java.nio.file.Path"]);
                let name = part.filename.as_deref().unwrap_or_else(|| basename(file));
                let content_type = part
                    .content_type
                    .as_deref()
                    .unwrap_or("application/octet-stream");
                main.push(add(format!(
                    "{disposition}; filename=\"{name}\"\r\nContent-Type: {content_type}\r\n\r\n"
                )));
                main.push(format!(
                    "{pad}form.add(Files.readAllBytes(Path.of({})));",
                    quote(file)
                ));
                main.push(format!(
                    r#"{pad}form.add("\r\n".getBytes(StandardCharsets.UTF_8));"#
                ));
            }
            (None, None) => {}
        }
    }
    main.push(format!(
        r#"{pad}form.add(("--" + boundary + "--\r\n").getBytes(StandardCharsets.UTF_8));"#
    ));
}

fn java_string(text: &str, pad: &str) -> String {
    if text.contains('\n') && !text.contains('\\') && !text.contains("\"\"\"") {
        format!("\"\"\"\n{}\"\"\"", indent(text, pad))
    } else {
        quote(text)
    }
}

pub fn java(req: &Request) -> String {
    let mut imports = vec![
        "java.net.URI",
        "java.net.http.HttpClient",
        "java.net.http.HttpRequest",
        "java.net.http.HttpResponse",
    ];
    let mut main = Vec::new();
    let pad = "        ";
    if req.insecure {
        main.push(format!("{pad}// curl -k: java.net.http has no switch to skip TLS verification; configure an SSLContext if needed"));
    }
    let proxy = req.proxy.as_deref().map(|p| (p, host_port(p)));
    if let Some((p, None)) = proxy {
        main.push(format!(
            "{pad}// curl -x {p}: could not read the proxy host and port"
        ));
    }
    let mut builder = Vec::new();
    if req.follow_redirects {
        builder.push(format!(
            "{pad}    .followRedirects(HttpClient.Redirect.NORMAL)"
        ));
    }
    if let Some((_, Some((host, port)))) = &proxy {
        imports.extend(["java.net.InetSocketAddress", "java.net.ProxySelector"]);
        builder.push(format!(
            "{pad}    .proxy(ProxySelector.of(new InetSocketAddress({}, {port})))",
            quote(host)
        ));
    }
    if builder.is_empty() {
        main.push(format!(
            "{pad}HttpClient client = HttpClient.newHttpClient();"
        ));
    } else {
        main.push(format!("{pad}HttpClient client = HttpClient.newBuilder()"));
        main.extend(builder);
        main.push(format!("{pad}    .build();"));
    }
    for (name, _) in headers(req) {
        if JAVA_RESTRICTED.contains(&name.to_ascii_lowercase().as_str()) {
            main.push(format!(
                "{pad}// header {} is set by java.net.http and cannot be overridden",
                quote(name)
            ));
        }
    }
    if is_multipart(req) {
        java_multipart(parts(req), pad, &mut imports, &mut main);
    }
    main.push(format!(
        "{pad}HttpRequest request = HttpRequest.newBuilder()"
    ));
    main.push(format!("{pad}    .uri(URI.create({}))", quote(&req.url)));
    if let Some(timeout) = req.timeout {
        imports.push("java.time.Duration");
        main.push(format!(
            "{pad}    .timeout(Duration.ofMillis({}))",
            millis(timeout)
        ));
    }
    let publisher = match (&req.body, json_body(req)) {
        (_, Some(value)) => Some(format!(
            "HttpRequest.BodyPublishers.ofString({})",
            java_string(&pretty(&value), &format!("{pad}        "))
        )),
        (Body::Text { text }, None) => Some(format!(
            "HttpRequest.BodyPublishers.ofString({})",
            java_string(text, &format!("{pad}        "))
        )),
        (Body::File { path }, None) => {
            imports.push("java.nio.file.Path");
            Some(format!(
                "HttpRequest.BodyPublishers.ofFile(Path.of({}))",
                quote(path)
            ))
        }
        (Body::Multipart { .. }, None) => {
            Some("HttpRequest.BodyPublishers.ofByteArrays(form)".to_owned())
        }
        (Body::None, None) => None,
    };
    match (req.method.as_str(), publisher) {
        ("GET", None) => {}
        (method, None) => main.push(format!(
            "{pad}    .method({}, HttpRequest.BodyPublishers.noBody())",
            quote(method)
        )),
        (method, Some(publisher)) => {
            main.push(format!("{pad}    .method({}, {publisher})", quote(method)))
        }
    }
    for (name, value) in headers(req) {
        if !JAVA_RESTRICTED.contains(&name.to_ascii_lowercase().as_str()) {
            main.push(format!(
                "{pad}    .header({}, {})",
                quote(name),
                quote(value)
            ));
        }
    }
    if is_multipart(req) {
        main.push(format!(
            r#"{pad}    .header("Content-Type", "multipart/form-data; boundary=" + boundary)"#
        ));
    }
    main.push(format!("{pad}    .build();"));
    main.push(format!("{pad}HttpResponse<String> response = client.send(request, HttpResponse.BodyHandlers.ofString());"));
    main.push(format!("{pad}System.out.println(response.statusCode());"));
    main.push(format!("{pad}System.out.println(response.body());"));

    imports.sort_unstable();
    imports.dedup();
    let mut out: Vec<String> = imports.iter().map(|i| format!("import {i};")).collect();
    out.push(String::new());
    out.push("public class Main {".into());
    out.push("    public static void main(String[] args) throws Exception {".into());
    out.extend(main);
    out.push("    }".into());
    out.push("}".into());
    out.join("\n") + "\n"
}

fn php_string(text: &str) -> String {
    format!("'{}'", text.replace('\\', "\\\\").replace('\'', "\\'"))
}

pub fn php(req: &Request) -> String {
    let mut out = vec![
        "<?php".to_owned(),
        String::new(),
        "$ch = curl_init();".into(),
    ];
    let set = |option: &str, value: String| format!("curl_setopt($ch, {option}, {value});");
    out.push(set("CURLOPT_URL", php_string(&req.url)));
    out.push(set("CURLOPT_RETURNTRANSFER", "true".into()));
    match req.method.as_str() {
        "GET" => {}
        "HEAD" => out.push(set("CURLOPT_NOBODY", "true".into())),
        "POST" if !matches!(req.body, Body::None) => {}
        method => out.push(set("CURLOPT_CUSTOMREQUEST", php_string(method))),
    }
    let list = headers(req);
    if !list.is_empty() {
        out.push("curl_setopt($ch, CURLOPT_HTTPHEADER, [".into());
        for (k, v) in list {
            out.push(format!("    {},", php_string(&format!("{k}: {v}"))));
        }
        out.push("]);".into());
    }
    match (&req.body, json_body(req)) {
        (_, Some(value)) => {
            let text = pretty(&value);
            let literal = if text.lines().any(|l| l.trim() == "JSON") {
                php_string(&text)
            } else {
                format!("<<<'JSON'\n{}\n    JSON", indent(&text, "    "))
            };
            out.push(set("CURLOPT_POSTFIELDS", literal));
        }
        (Body::Text { text }, None) => out.push(set("CURLOPT_POSTFIELDS", php_string(text))),
        (Body::File { path }, None) => out.push(set(
            "CURLOPT_POSTFIELDS",
            format!("file_get_contents({})", php_string(path)),
        )),
        (Body::Multipart { parts }, None) => {
            out.push("curl_setopt($ch, CURLOPT_POSTFIELDS, [".into());
            for part in parts {
                let value = match (&part.value, &part.file) {
                    (Some(value), _) => php_string(value),
                    (None, Some(file)) => {
                        let mut args = vec![php_string(file)];
                        if part.content_type.is_some() || part.filename.is_some() {
                            args.push(part.content_type.as_deref().map_or("''".into(), php_string));
                        }
                        if let Some(filename) = &part.filename {
                            args.push(php_string(filename));
                        }
                        format!("new CURLFile({})", args.join(", "))
                    }
                    (None, None) => continue,
                };
                out.push(format!("    {} => {value},", php_string(&part.name)));
            }
            out.push("]);".into());
        }
        (Body::None, None) => {}
    }
    if req.follow_redirects {
        out.push(set("CURLOPT_FOLLOWLOCATION", "true".into()));
    }
    if req.insecure {
        out.push(set("CURLOPT_SSL_VERIFYPEER", "false".into()));
        out.push(set("CURLOPT_SSL_VERIFYHOST", "0".into()));
    }
    if let Some(timeout) = req.timeout {
        out.push(set("CURLOPT_TIMEOUT_MS", millis(timeout).to_string()));
    }
    if let Some(proxy) = &req.proxy {
        out.push(set("CURLOPT_PROXY", php_string(proxy)));
    }
    if req.compressed {
        out.push(set("CURLOPT_ENCODING", "''".into()));
    }
    out.push(String::new());
    out.push("$response = curl_exec($ch);".into());
    out.push("if ($response === false) {".into());
    out.push("    throw new RuntimeException(curl_error($ch));".into());
    out.push("}".into());
    out.push("echo curl_getinfo($ch, CURLINFO_RESPONSE_CODE), PHP_EOL;".into());
    out.push("echo $response, PHP_EOL;".into());
    out.join("\n") + "\n"
}

fn csharp_string(text: &str) -> String {
    if text.contains('\n') && !text.contains("\"\"\"") {
        format!("\"\"\"\n{}\n\"\"\"", text)
    } else {
        quote(text)
    }
}

pub fn csharp(req: &Request) -> String {
    let mut out = Vec::new();
    let mut handler = Vec::new();
    if req.insecure {
        handler.push("    ServerCertificateCustomValidationCallback = HttpClientHandler.DangerousAcceptAnyServerCertificateValidator,".to_owned());
    }
    if let Some(proxy) = &req.proxy {
        handler.push(format!("    Proxy = new WebProxy({}),", quote(proxy)));
        handler.push("    UseProxy = true,".into());
    }
    if req.compressed {
        handler.push("    AutomaticDecompression = DecompressionMethods.All,".into());
    }
    if req.proxy.is_some() || req.compressed {
        out.push("using System.Net;".to_owned());
        out.push(String::new());
    }
    let timeout = req
        .timeout
        .map(|t| format!(" {{ Timeout = TimeSpan.FromMilliseconds({}) }}", millis(t)))
        .unwrap_or_default();
    if handler.is_empty() {
        out.push(format!("using var client = new HttpClient(){timeout};"));
    } else {
        out.push("var handler = new HttpClientHandler".into());
        out.push("{".into());
        out.extend(handler);
        out.push("};".into());
        out.push(format!(
            "using var client = new HttpClient(handler){timeout};"
        ));
    }
    let method = match req.method.as_str() {
        "GET" => "HttpMethod.Get".to_owned(),
        "POST" => "HttpMethod.Post".to_owned(),
        "PUT" => "HttpMethod.Put".to_owned(),
        "PATCH" => "HttpMethod.Patch".to_owned(),
        "DELETE" => "HttpMethod.Delete".to_owned(),
        "HEAD" => "HttpMethod.Head".to_owned(),
        "OPTIONS" => "HttpMethod.Options".to_owned(),
        other => format!("new HttpMethod({})", quote(other)),
    };
    out.push(format!(
        "using var request = new HttpRequestMessage({method}, {});",
        quote(&req.url)
    ));

    let content = match (&req.body, json_body(req)) {
        (_, Some(value)) => Some(format!(
            "request.Content = new StringContent({});",
            csharp_string(&pretty(&value))
        )),
        (Body::Text { text }, None) => Some(format!(
            "request.Content = new StringContent({});",
            csharp_string(text)
        )),
        (Body::File { path }, None) => Some(format!(
            "request.Content = new StreamContent(File.OpenRead({}));",
            quote(path)
        )),
        (Body::Multipart { parts }, None) => {
            out.push("var form = new MultipartFormDataContent();".into());
            for (i, part) in parts.iter().enumerate() {
                match (&part.value, &part.file) {
                    (Some(value), _) => out.push(format!(
                        "form.Add(new StringContent({}), {});",
                        quote(value),
                        quote(&part.name)
                    )),
                    (None, Some(file)) => {
                        let n = i + 1;
                        let name = part.filename.as_deref().unwrap_or_else(|| basename(file));
                        out.push(format!(
                            "var file{n} = new StreamContent(File.OpenRead({}));",
                            quote(file)
                        ));
                        if let Some(t) = &part.content_type {
                            out.push(format!(
                                "file{n}.Headers.TryAddWithoutValidation(\"Content-Type\", {});",
                                quote(t)
                            ));
                        }
                        out.push(format!(
                            "form.Add(file{n}, {}, {});",
                            quote(&part.name),
                            quote(name)
                        ));
                    }
                    (None, None) => {}
                }
            }
            Some("request.Content = form;".to_owned())
        }
        (Body::None, None) => None,
    };
    let has_content = content.is_some();
    if let Some(line) = content {
        out.push(line);
    }
    for (name, value) in headers(req) {
        let lower = name.to_ascii_lowercase();
        if lower.starts_with("content-") {
            if !has_content {
                out.push(format!(
                    "// header {} needs a request body and was skipped",
                    quote(name)
                ));
                continue;
            }
            if lower == "content-type" {
                out.push("request.Content.Headers.Remove(\"Content-Type\");".into());
            }
            out.push(format!(
                "request.Content.Headers.TryAddWithoutValidation({}, {});",
                quote(name),
                quote(value)
            ));
        } else {
            out.push(format!(
                "request.Headers.TryAddWithoutValidation({}, {});",
                quote(name),
                quote(value)
            ));
        }
    }
    out.push(String::new());
    out.push("using var response = await client.SendAsync(request);".into());
    out.push("Console.WriteLine((int)response.StatusCode);".into());
    out.push("Console.WriteLine(await response.Content.ReadAsStringAsync());".into());
    out.join("\n") + "\n"
}

#[cfg(test)]
#[path = "emit_test.rs"]
mod tests;
