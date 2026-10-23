use std::collections::BTreeMap;
use std::path::PathBuf;

use super::*;

fn temp_installed(name: &str) -> (Installed, PathBuf) {
    let root = std::env::temp_dir().join(format!("tfp-catalog-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    (Installed::new(root.clone(), "0.2.0"), root)
}

/// 一个声明了 CLI 接口的在线插件包（WASM 内容无效，只用于列出与报错）
fn package(id: &str, min_app: &str) -> Vec<u8> {
    let manifest = format!(
        r#"{{"id":"{id}","version":"1.0.0","minAppVersion":"{min_app}","name":"i18n:name","description":"d","category":"dev",
            "functions":{{"run":{{"interface":{{"expose":["cli"],"summary":"s","effect":"read","params":{{"type":"object"}}}}}}}}}}"#
    );
    let files = BTreeMap::from([
        ("manifest.json".to_owned(), manifest.into_bytes()),
        ("plugin.wasm".to_owned(), b"\0asm".to_vec()),
        (
            "ui/index.js".to_owned(),
            b"export default () => null".to_vec(),
        ),
        (
            "locales/en-US.json".to_owned(),
            br#"{"name":"Demo"}"#.to_vec(),
        ),
    ]);
    tf_package::build(&files, None).unwrap()
}

#[test]
fn slug_is_the_last_id_segment() {
    assert_eq!(slug("org.devlive.toolforge.hash"), "hash");
    assert_eq!(slug("plain"), "plain");
}

#[test]
fn loads_builtin_plugins_with_their_locale() {
    let catalog = Catalog::new(builtin::all(), None);
    let loaded = catalog.load("hash", "zh-CN").unwrap();
    assert_eq!(loaded.plugin.manifest().id, "org.devlive.toolforge.hash");
    assert!(loaded.locale["api"]["hash_text"]["summary"].is_string());
    assert_eq!(
        catalog.load("nothing", "en-US").err().unwrap().code,
        "cli.unknown_plugin"
    );
}

#[test]
fn lists_installed_plugins_and_reports_their_load_errors() {
    let (installed, root) = temp_installed("list");
    installed
        .install(&package("org.example.demo", "0.1.0"), None, true, |_| false)
        .unwrap();
    let catalog = Catalog::new(builtin::all(), Some(installed));
    let listed = catalog.manifests("en-US");
    let names: Vec<&str> = listed.iter().map(|(m, _)| slug(&m.id)).collect();
    assert_eq!(names, ["demo", "hash"]);
    assert_eq!(
        listed[0].1["name"], "Demo",
        "installed locales are read without loading"
    );
    let err = catalog.load("demo", "en-US").err().unwrap();
    assert_ne!(
        err.code, "cli.unknown_plugin",
        "found but the fake component cannot load"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn installed_plugins_needing_a_newer_app_are_refused() {
    let (installed, root) = temp_installed("needs-app");
    let newer = Installed::new(root.clone(), "9.0.0");
    newer
        .install(&package("org.example.future", "9.0.0"), None, true, |_| {
            false
        })
        .unwrap();
    let catalog = Catalog::new(Vec::new(), Some(installed));
    assert_eq!(
        catalog.load("future", "en-US").err().unwrap().code,
        "market.needs_app"
    );
    std::fs::remove_dir_all(root).unwrap();
}
