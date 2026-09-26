use super::*;

fn site() -> (Config, PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "tfp-server-{}-{}",
        std::process::id(),
        rand_suffix()
    ));
    std::fs::create_dir_all(dir.join("docs/nested")).unwrap();
    std::fs::create_dir_all(dir.join(".git")).unwrap();
    std::fs::create_dir_all(dir.join("empty")).unwrap();
    std::fs::write(dir.join("index.html"), "<h1>home</h1>").unwrap();
    std::fs::write(dir.join("app.js"), "console.log(1)").unwrap();
    std::fs::write(dir.join("data.bin"), (0..100u8).collect::<Vec<_>>()).unwrap();
    std::fs::write(dir.join("docs/readme.md"), "# docs").unwrap();
    std::fs::write(dir.join("docs/a <b>.txt"), "x").unwrap();
    std::fs::write(dir.join(".env"), "SECRET=1").unwrap();
    std::fs::write(dir.join(".git/config"), "x").unwrap();
    let root = dir.canonicalize().unwrap();
    let config = Config {
        root: root.clone(),
        listing: true,
        spa: false,
        cors: false,
        hide_dotfiles: true,
    };
    (config, root)
}

/// 每个测试使用独立目录；macOS 时钟只有微秒精度，时间戳作为目录名会在并行测试中冲突
fn rand_suffix() -> usize {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn header<'a>(reply: &'a Reply, name: &str) -> Option<&'a str> {
    reply
        .headers
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

#[test]
fn serves_files_with_types() {
    let (config, root) = site();
    let reply = handle(&config, "GET", "/app.js?v=1", None);
    assert_eq!(reply.status, 200);
    assert_eq!(reply.body, Body::File(root.join("app.js"), 0, 14));
    assert_eq!(
        header(&reply, "Content-Type"),
        Some("text/javascript; charset=utf-8")
    );
    assert_eq!(header(&reply, "Accept-Ranges"), Some("bytes"));
    assert_eq!(
        header(&reply, "Content-Type").map(|_| handle(&config, "HEAD", "/data.bin", None).status),
        Some(200)
    );
    assert_eq!(
        handle(&config, "GET", "/", None).body,
        Body::File(root.join("index.html"), 0, 13)
    );
    assert_eq!(handle(&config, "GET", "/missing.css", None).status, 404);
}

#[test]
fn lists_directories_and_redirects_to_trailing_slash() {
    let (config, _) = site();
    let redirect = handle(&config, "GET", "/docs", None);
    assert_eq!(
        (redirect.status, header(&redirect, "Location")),
        (301, Some("/docs/"))
    );
    let reply = handle(&config, "GET", "/docs/", None);
    let Body::Text(html) = &reply.body else {
        panic!("{reply:?}")
    };
    assert!(
        html.contains("href=\"nested/\"") && html.contains("href=\"readme%2Emd\""),
        "{html}"
    );
    assert!(
        html.contains("a &lt;b&gt;.txt"),
        "names are escaped: {html}"
    );
    assert!(html.contains("href=\"../\""));
    let off = Config {
        listing: false,
        ..config
    };
    assert_eq!(handle(&off, "GET", "/empty/", None).status, 403);
}

#[test]
fn blocks_traversal_and_hidden_files() {
    let (config, _) = site();
    for url in [
        "/../etc/passwd",
        "/docs/../../x",
        "/%2e%2e/secret",
        "/..%2f..%2fetc",
        "/a\\..\\b",
    ] {
        assert_eq!(handle(&config, "GET", url, None).status, 400, "{url}");
    }
    assert_eq!(handle(&config, "GET", "/.env", None).status, 404);
    assert_eq!(handle(&config, "GET", "/.git/config", None).status, 404);
    std::fs::remove_file(config.root.join("index.html")).unwrap();
    let Body::Text(html) = handle(&config, "GET", "/", None).body else {
        panic!("root without index.html is listed")
    };
    assert!(
        html.contains("app.js") && !html.contains(".env") && !html.contains(".git"),
        "{html}"
    );
}

#[cfg(unix)]
#[test]
fn blocks_symlinks_that_leave_the_root() {
    let (config, root) = site();
    std::os::unix::fs::symlink("/etc", root.join("outside")).unwrap();
    assert_eq!(handle(&config, "GET", "/outside/hosts", None).status, 403);
}

#[test]
fn shows_dotfiles_when_allowed() {
    let (config, _) = site();
    let open = Config {
        hide_dotfiles: false,
        ..config
    };
    assert_eq!(handle(&open, "GET", "/.env", None).status, 200);
}

#[test]
fn supports_ranges() {
    let (config, root) = site();
    let reply = handle(&config, "GET", "/data.bin", Some("bytes=10-19"));
    assert_eq!(reply.status, 206);
    assert_eq!(reply.body, Body::File(root.join("data.bin"), 10, 10));
    assert_eq!(header(&reply, "Content-Range"), Some("bytes 10-19/100"));
    assert_eq!(parse_range("bytes=-5", 100), Some((95, 99)));
    assert_eq!(parse_range("bytes=90-", 100), Some((90, 99)));
    assert_eq!(parse_range("bytes=90-500", 100), Some((90, 99)));
    assert_eq!(parse_range("bytes=0-1,5-6", 100), None);
    assert_eq!(
        handle(&config, "GET", "/data.bin", Some("bytes=200-300")).status,
        416
    );
}

#[test]
fn falls_back_for_single_page_apps_and_handles_cors() {
    let (config, root) = site();
    let spa = Config {
        spa: true,
        cors: true,
        ..config.clone()
    };
    assert_eq!(
        handle(&spa, "GET", "/users/42", None).body,
        Body::File(root.join("index.html"), 0, 13)
    );
    assert_eq!(handle(&spa, "GET", "/missing.js", None).status, 404);
    assert_eq!(handle(&config, "GET", "/users/42", None).status, 404);
    let reply = handle(&spa, "GET", "/app.js", None);
    assert_eq!(header(&reply, "Access-Control-Allow-Origin"), Some("*"));
    assert_eq!(handle(&spa, "OPTIONS", "/app.js", None).status, 204);
    assert_eq!(handle(&config, "POST", "/app.js", None).status, 405);
}
