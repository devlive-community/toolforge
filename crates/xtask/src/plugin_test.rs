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

    let out = index(
        &root,
        "https://example.com",
        None,
        None,
        &"a".repeat(40),
        1,
        &Default::default(),
    )
    .unwrap();
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
        "https://example.com/v1/plugins/org.toolforge.test.a/versions/0.1.10/download"
    );
    let b = plugins
        .iter()
        .find(|p| p["id"] == "org.toolforge.test.b")
        .unwrap();
    assert_eq!(b["version"], "0.1.0");

    let catalog: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("release-catalog.json")).unwrap()).unwrap();
    assert_eq!(catalog["plugins"].as_array().unwrap().len(), 4);
    assert_eq!(catalog["generation"], 1);
    for item in catalog["plugins"].as_array().unwrap() {
        assert_eq!(
            item["objectKey"],
            format!(
                "packages/{}/{}/{}.tfp",
                item["id"].as_str().unwrap(),
                item["version"].as_str().unwrap(),
                item["sha256"].as_str().unwrap()
            )
        );
    }
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn index_rejects_same_version_with_different_bytes() {
    let root = std::env::temp_dir().join(format!("xtask-plugin-conflict-{}", std::process::id()));
    std::fs::create_dir_all(root.join("previous")).unwrap();
    std::fs::write(
        root.join("a.tfp"),
        pack_bytes("org.toolforge.test.a", "1.0.0", "red"),
    )
    .unwrap();
    std::fs::write(
        root.join("previous/a.tfp"),
        pack_bytes("org.toolforge.test.a", "1.0.0", "blue"),
    )
    .unwrap();
    assert!(
        index(
            &root,
            "https://example.com",
            None,
            None,
            &"a".repeat(40),
            1,
            &Default::default()
        )
        .unwrap_err()
        .contains("conflicting")
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn index_preserves_revocation_and_requires_complete_inventory() {
    let root = std::env::temp_dir().join(format!("xtask-plugin-catalog-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let package = root.join("a.tfp");
    std::fs::write(&package, pack_bytes("org.toolforge.test.a", "1.0.0", "red")).unwrap();
    index(
        &root,
        "https://example.com",
        None,
        None,
        &"a".repeat(40),
        1,
        &Default::default(),
    )
    .unwrap();
    let previous = root.join("old-catalog.json");
    let mut catalog: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("release-catalog.json")).unwrap()).unwrap();
    catalog["plugins"][0]["status"] = "revoked".into();
    std::fs::write(&previous, serde_json::to_vec(&catalog).unwrap()).unwrap();
    let registry = index(
        &root,
        "https://example.com",
        None,
        Some(&previous),
        &"b".repeat(40),
        2,
        &Default::default(),
    )
    .unwrap();
    let registry: serde_json::Value =
        serde_json::from_slice(&std::fs::read(registry).unwrap()).unwrap();
    assert!(registry["plugins"].as_array().unwrap().is_empty());
    std::fs::remove_file(package).unwrap();
    assert!(
        index(
            &root,
            "https://example.com",
            None,
            Some(&previous),
            &"c".repeat(40),
            3,
            &Default::default()
        )
        .unwrap_err()
        .contains("missing")
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn index_allows_only_identical_same_generation_retry() {
    let root = std::env::temp_dir().join(format!("xtask-plugin-retry-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("a.tfp"),
        pack_bytes("org.toolforge.test.a", "1.0.0", "red"),
    )
    .unwrap();
    index(
        &root,
        "https://example.com",
        None,
        None,
        &"a".repeat(40),
        7,
        &Default::default(),
    )
    .unwrap();
    let previous = root.join("old-catalog.json");
    std::fs::copy(root.join("release-catalog.json"), &previous).unwrap();
    index(
        &root,
        "https://example.com",
        None,
        Some(&previous),
        &"a".repeat(40),
        7,
        &Default::default(),
    )
    .unwrap();
    assert!(
        index(
            &root,
            "https://example.com",
            None,
            Some(&previous),
            &"b".repeat(40),
            7,
            &Default::default()
        )
        .unwrap_err()
        .contains("different content")
    );
    assert!(
        index(
            &root,
            "https://example.com",
            None,
            Some(&previous),
            &"a".repeat(40),
            6,
            &Default::default()
        )
        .unwrap_err()
        .contains("backwards")
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn channels_merge_function_interfaces() {
    let manifest = serde_json::json!({
        "functions": {
            "a": { "interface": { "expose": ["cli"] } },
            "b": { "interface": { "expose": ["agent", "cli", "future"] } },
            "c": { "interface": false },
            "d": {}
        }
    });
    assert_eq!(channels(&manifest), ["agent", "cli"]);
    assert!(channels(&serde_json::json!({})).is_empty());
}

#[test]
fn cargo_pins_rustc_from_the_same_toolchain() {
    let command = cargo();
    let rustc = command
        .get_envs()
        .find(|(key, _)| *key == "RUSTC")
        .and_then(|(_, value)| value);
    if let Some(rustc) = rustc {
        assert_eq!(Path::new(rustc).parent(), Path::new(env!("CARGO")).parent());
    }
}

#[test]
fn index_records_first_release_and_update_times() {
    let root = std::env::temp_dir().join(format!("xtask-plugin-dates-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    for version in ["0.1.0", "0.1.1"] {
        std::fs::write(
            root.join(format!("a-{version}.tfp")),
            pack_bytes("org.toolforge.test.a", version, "red"),
        )
        .unwrap();
    }
    let mut dates = crate::release_dates::ReleaseDates::default();
    dates.versions.insert(
        ("org.toolforge.test.a".into(), "0.1.0".into()),
        "2026-10-01T00:00:00Z".into(),
    );
    // 0.1.1 没有历史记录，使用本次发布时间
    dates.now = Some("2026-10-20T00:00:00Z".into());
    let out = index(
        &root,
        "https://example.com",
        None,
        None,
        &"a".repeat(40),
        1,
        &dates,
    )
    .unwrap();
    let registry: serde_json::Value = serde_json::from_slice(&std::fs::read(out).unwrap()).unwrap();
    let entry = &registry["plugins"][0];
    assert_eq!(entry["version"], "0.1.1");
    assert_eq!(entry["publishedAt"], "2026-10-01T00:00:00Z");
    assert_eq!(entry["updatedAt"], "2026-10-20T00:00:00Z");

    // 再次发布时以旧目录中的时间为准，不被新的发布时间改写
    let previous = root.join("old-catalog.json");
    std::fs::copy(root.join("release-catalog.json"), &previous).unwrap();
    let later = crate::release_dates::ReleaseDates {
        versions: Default::default(),
        now: Some("2026-12-01T00:00:00Z".into()),
    };
    let out = index(
        &root,
        "https://example.com",
        None,
        Some(&previous),
        &"b".repeat(40),
        2,
        &later,
    )
    .unwrap();
    let registry: serde_json::Value = serde_json::from_slice(&std::fs::read(out).unwrap()).unwrap();
    assert_eq!(registry["plugins"][0]["updatedAt"], "2026-10-20T00:00:00Z");
    assert_eq!(
        registry["plugins"][0]["publishedAt"],
        "2026-10-01T00:00:00Z"
    );
    std::fs::remove_dir_all(root).unwrap();
}
