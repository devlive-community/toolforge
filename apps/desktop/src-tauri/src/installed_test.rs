use super::*;

fn package(id: &str, version: &str) -> Vec<u8> {
    let manifest = format!(
        r#"{{"id":"{id}","version":"{version}","name":"i18n:name","description":"i18n:description","category":"text","functions":{{"run":{{}}}}}}"#
    );
    let files = BTreeMap::from([
        ("manifest.json".to_owned(), manifest.into_bytes()),
        ("plugin.wasm".to_owned(), b"\0asm".to_vec()),
        (
            "ui/index.js".to_owned(),
            b"export default () => null".to_vec(),
        ),
        ("ui/index.css".to_owned(), b".x{}".to_vec()),
        ("icon.svg".to_owned(), b"<svg/>".to_vec()),
        (
            "locales/zh-CN.json".to_owned(),
            r#"{"name":"示例"}"#.as_bytes().to_vec(),
        ),
    ]);
    tf_package::build(&files, None).unwrap()
}

fn temp(name: &str) -> Installed {
    let root = std::env::temp_dir().join(format!("tf-installed-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    Installed::new(root)
}

#[test]
fn unsigned_packages_need_confirmation() {
    let installed = temp("unsigned");
    let err = installed
        .install(&package("org.example.demo", "1.0.0"), None, false, |_| {
            false
        })
        .unwrap_err();
    assert_eq!(err.code, "package.untrusted");
    assert_eq!(err.params["trust"], "unsigned");
    assert!(installed.list().is_empty());

    let (info, manifest) = installed
        .install(
            &package("org.example.demo", "1.0.0"),
            Some("https://example.org/demo.tfp"),
            true,
            |_| false,
        )
        .unwrap();
    assert_eq!(
        (info.trust.as_str(), manifest.version.as_str()),
        ("unsigned", "1.0.0")
    );
    assert_eq!(installed.list(), vec![info]);
    std::fs::remove_dir_all(installed.root()).unwrap();
}

#[test]
fn updates_replace_files_and_uninstall_removes_them() {
    let installed = temp("update");
    installed
        .install(&package("org.example.demo", "1.0.0"), None, true, |_| false)
        .unwrap();
    installed
        .install(&package("org.example.demo", "1.1.0"), None, true, |_| false)
        .unwrap();
    let list = installed.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].version, "1.1.0");
    assert_eq!(
        installed.manifest("org.example.demo").unwrap().version,
        "1.1.0"
    );
    // 不留下临时目录
    let leftovers = std::fs::read_dir(installed.root()).unwrap().count();
    assert_eq!(leftovers, 1);
    installed.uninstall("org.example.demo").unwrap();
    assert!(installed.list().is_empty());
    assert_eq!(
        installed.uninstall("org.example.demo").unwrap_err().code,
        "package.not_installed"
    );
    std::fs::remove_dir_all(installed.root()).unwrap();
}

#[test]
fn rejects_builtin_ids_and_serves_only_ui_files() {
    let installed = temp("serve");
    let err = installed
        .install(
            &package("org.devlive.toolforge.uuid", "9.0.0"),
            None,
            true,
            |id| id == "org.devlive.toolforge.uuid",
        )
        .unwrap_err();
    assert_eq!(err.code, "package.builtin_conflict");

    installed
        .install(&package("org.example.demo", "1.0.0"), None, true, |_| false)
        .unwrap();
    assert_eq!(
        installed.file("org.example.demo", "ui/index.js").unwrap(),
        b"export default () => null"
    );
    assert_eq!(
        installed
            .file("org.example.demo", "plugin.wasm")
            .unwrap_err()
            .code,
        "fs.not_found"
    );
    assert_eq!(
        installed.file("org.example.demo", "../x").unwrap_err().code,
        "fs.not_found"
    );
    assert_eq!(
        installed.file("../etc", "ui/index.js").unwrap_err().code,
        "package.bad_id"
    );

    let assets = installed.assets("org.example.demo").unwrap();
    assert_eq!(assets.locales["zh-CN"]["name"], "示例");
    assert_eq!(assets.icon.as_deref(), Some("<svg/>"));
    assert!(assets.css);
    std::fs::remove_dir_all(installed.root()).unwrap();
}

/// 端到端：`TOOLFORGE_TEST_TFP=<签名的 .tfp> cargo test -p toolforge installs_real_package -- --ignored`
#[test]
#[ignore]
fn installs_real_package() {
    let path = std::env::var("TOOLFORGE_TEST_TFP").expect("set TOOLFORGE_TEST_TFP");
    let installed = temp("real");
    let (info, manifest) = installed
        .install(&std::fs::read(path).unwrap(), None, false, |_| false)
        .unwrap();
    assert_eq!(info.trust, "official");
    let plugin = installed.load(&manifest.id).unwrap();
    let out = tf_plugin_api::ToolPlugin::call(
        plugin.as_ref(),
        "validate",
        serde_json::json!({ "input": "91350100M000100Y43" }),
    )
    .unwrap();
    assert_eq!(out["summary"]["valid"], 1);
    std::fs::remove_dir_all(installed.root()).unwrap();
}
