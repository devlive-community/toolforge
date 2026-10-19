use super::*;
use std::collections::BTreeMap;

#[test]
fn version_key_compares_numeric_segments() {
    assert!(version_key("0.1.10") > version_key("0.1.9"));
    assert!(version_key("0.2.0") > version_key("0.1.10"));
    assert!(version_key("1.0.0") > version_key("0.10.10"));
    assert_eq!(version_key("0.1.2"), version_key("0.1.2"));
}

fn pack_bytes(id: &str, version: &str, accent: &str) -> Vec<u8> {
    let manifest = serde_json::json!({
        "id": id,
        "version": version,
        "name": "i18n:name",
        "description": "i18n:description",
        "category": "dev",
        "accent": accent,
        "locales": ["en-US"],
    });
    let locale = serde_json::json!({
        "name": format!("{id} {version}"),
        "description": "test plugin",
    });
    let files = BTreeMap::from([
        (
            "manifest.json".to_owned(),
            serde_json::to_vec(&manifest).unwrap(),
        ),
        ("plugin.wasm".to_owned(), b"wasm".to_vec()),
        ("ui/index.js".to_owned(), b"export default {}".to_vec()),
        (
            "locales/en-US.json".to_owned(),
            serde_json::to_vec(&locale).unwrap(),
        ),
    ]);
    tf_package::build(&files, None).unwrap()
}

#[test]
fn index_keeps_only_the_highest_version_per_plugin() {
    let root = std::env::temp_dir().join(format!("xtask-plugin-index-{}", std::process::id()));
    let previous = root.join("previous");
    std::fs::create_dir_all(&previous).unwrap();
    // 历史版本在子目录，最新版本在根目录：文件名排序会把 0.1.10 排到 0.1.9 前
    std::fs::write(
        previous.join("org.toolforge.test.a-0.1.9.tfp"),
        pack_bytes("org.toolforge.test.a", "0.1.9", "green"),
    )
    .unwrap();
    std::fs::write(
        previous.join("org.toolforge.test.a-0.1.2.tfp"),
        pack_bytes("org.toolforge.test.a", "0.1.2", "green"),
    )
    .unwrap();
    std::fs::write(
        root.join("org.toolforge.test.a-0.1.10.tfp"),
        pack_bytes("org.toolforge.test.a", "0.1.10", "pink"),
    )
    .unwrap();
    std::fs::write(
        root.join("org.toolforge.test.b-0.1.0.tfp"),
        pack_bytes("org.toolforge.test.b", "0.1.0", "cyan"),
    )
    .unwrap();

    let out = index(&root, "https://example.com/download", None).unwrap();
    let data: serde_json::Value = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
    let plugins = data["plugins"].as_array().unwrap();
    assert_eq!(plugins.len(), 2);
    let a = plugins
        .iter()
        .find(|p| p["id"] == "org.toolforge.test.a")
        .unwrap();
    assert_eq!(a["version"], "0.1.10");
    // 条目来自最高版本包：accent 与本地化文案都取自 0.1.10
    assert_eq!(a["accent"], "pink");
    assert_eq!(a["names"]["en-US"], "org.toolforge.test.a 0.1.10");
    assert_eq!(
        a["url"],
        "https://example.com/download/org.toolforge.test.a-0.1.10.tfp"
    );
    let b = plugins
        .iter()
        .find(|p| p["id"] == "org.toolforge.test.b")
        .unwrap();
    assert_eq!(b["version"], "0.1.0");

    std::fs::remove_dir_all(&root).ok();
}
