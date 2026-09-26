use std::collections::BTreeSet;
use std::path::Path;

use super::*;

/// 每个 plugins/*/manifest.json 都必须在 builtin() 中注册，反之亦然。
/// 新增插件忘记注册时（Cargo 依赖存在、能编译），只有这里能发现。
#[test]
fn every_plugin_directory_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../plugins");
    let on_disk: BTreeSet<String> = std::fs::read_dir(&root)
        .expect("plugins directory")
        .filter_map(|entry| {
            let manifest = entry.ok()?.path().join("manifest.json");
            let raw = std::fs::read_to_string(manifest).ok()?;
            let value: serde_json::Value = serde_json::from_str(&raw).expect("valid manifest");
            Some(value["id"].as_str()?.to_owned())
        })
        .collect();
    let registered: BTreeSet<String> = builtin().manifests().into_iter().map(|m| m.id).collect();

    let missing: Vec<_> = on_disk.difference(&registered).collect();
    let unknown: Vec<_> = registered.difference(&on_disk).collect();
    assert!(
        missing.is_empty(),
        "plugins not registered in builtin(): {missing:?}"
    );
    assert!(
        unknown.is_empty(),
        "registered plugins without a directory: {unknown:?}"
    );
}

/// 常见剪贴板内容应当把最合适的工具排在第一位
#[test]
fn clipboard_samples_suggest_the_right_tool_first() {
    let registry = builtin();
    let cases = [
        ("{\"name\": \"Ada\", \"tags\": [1, 2]}", "json-formatter"),
        ("[1, 2, 3]", "json-formatter"),
        (
            "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxIn0.c2ln",
            "jwt",
        ),
        ("1700000000", "timestamp"),
        ("1700000000123", "timestamp"),
        ("2024-02-29 12:00:00", "timestamp"),
        ("550e8400-e29b-41d4-a716-446655440000", "uuid"),
        ("#16a34a", "color"),
        ("rgb(22 163 74)", "color"),
        ("q%3Dhello%20world", "encoder"),
        ("VG9vbEZvcmdlIOW3peWFt+eusQ==", "encoder"),
        (
            "curl -X POST https://api.example.com -d a=1",
            "curl-converter",
        ),
        ("30 9 * * MON-FRI", "cron"),
        ("192.168.1.0/24", "ip-calculator"),
        ("10.0.0.1", "ip-calculator"),
        ("<a><b>1</b></a>", "xml-formatter"),
        ("SELECT id FROM users WHERE id = 1", "sql-formatter"),
        ("0xDEADBEEF", "base-converter"),
        ("https://api.github.com/repos/x", "http-client"),
        ("github.com", "dns-lookup"),
        ("name\tage\nAda\t36\n", "csv-viewer"),
        ("name: app\nversion: 1\n", "format-converter"),
        ("localhost:3000", "port-manager"),
        (
            "-----BEGIN PUBLIC KEY-----\nMIIBIjAN\n-----END PUBLIC KEY-----",
            "crypto",
        ),
        (
            include_str!("../../../../plugins/certificate/backend/fixtures/chain.pem"),
            "certificate",
        ),
        ("db.internal:5432", "port-manager"),
        ("[server]\nport = 80\n", "format-converter"),
    ];
    for (text, expected) in cases {
        let suggestions = registry.detect(text);
        let first = suggestions.first().map(|s| s.plugin_id.as_str());
        assert_eq!(
            first,
            Some(format!("org.devlive.toolforge.{expected}").as_str()),
            "{text:?} → {suggestions:?}"
        );
    }
    for text in [
        "hello world",
        "readme.md",
        "12345",
        "d41d8cd98f00b204e9800998ecf8427e",
        "Ada Lovelace, 1815",
    ] {
        assert!(
            registry.detect(text).is_empty(),
            "{text:?} → {:?}",
            registry.detect(text)
        );
    }
}

/// 剪贴板中是图片时推荐文字识别与二维码识别
#[test]
fn clipboard_images_suggest_image_tools() {
    let registry = builtin();
    let ids: Vec<String> = registry
        .detect_image(1440, 900)
        .into_iter()
        .map(|s| s.plugin_id)
        .collect();
    assert_eq!(
        ids,
        vec!["org.devlive.toolforge.ocr", "org.devlive.toolforge.qrcode"]
    );
    assert!(registry.detect_image(8, 8).is_empty());
}
