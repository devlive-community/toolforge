use crate::mask;

#[test]
fn masks_password_in_urls_and_keeps_plain_hosts() {
    assert_eq!(
        mask::mask_url("http://user:secret@proxy.corp:8080"),
        "http://user:••••@proxy.corp:8080"
    );
    // 无 userinfo 原样
    assert_eq!(
        mask::mask_url("http://proxy.corp:8080/pac.pac"),
        "http://proxy.corp:8080/pac.pac"
    );
    // 路径中的 @ 不当作 userinfo
    assert_eq!(
        mask::mask_url("http://proxy.corp/a@b"),
        "http://proxy.corp/a@b"
    );
    // 无 scheme 原样
    assert_eq!(mask::mask_url("proxy.corp:8080"), "proxy.corp:8080");
    assert_eq!(
        mask::mask_host("user:secret@10.0.0.1"),
        "user:••••@10.0.0.1"
    );
    assert_eq!(mask::mask_host("10.0.0.1"), "10.0.0.1");
    // 大小写 scheme 不影响
    assert_eq!(
        mask::mask_url("SOCKS5://u:p@h:1080"),
        "SOCKS5://u:••••@h:1080"
    );
}

#[test]
fn masks_raw_json_and_truncates() {
    let raw = serde_json::json!({
        "url": "http://alice:pw@host/pac",
        "plain": "localhost",
        "list": ["http://a:b@c", "ok"],
        "long": "x".repeat(600),
    });
    let masked = mask::mask_raw(&raw);
    assert_eq!(masked["url"], "http://alice:••••@host/pac");
    assert_eq!(masked["plain"], "localhost");
    assert_eq!(masked["list"][0], "http://a:••••@c");
    assert_eq!(masked["list"][1], "ok");
    // 超长截断到 512 字符 + 三字节省略号
    assert_eq!(masked["long"].as_str().unwrap().len(), 515);
    // 非字符串标量原样
    assert_eq!(mask::mask_raw(&serde_json::json!(5)), serde_json::json!(5));
    // 数组截断到 64 项
    let items: Vec<i32> = (0..100).collect();
    assert_eq!(
        mask::mask_raw(&serde_json::json!(items))
            .as_array()
            .unwrap()
            .len(),
        64
    );
}
