use super::*;

fn package(id: &str, version: &str) -> Vec<u8> {
    package_with(id, version, "")
}

/// extra 为追加到 manifest 的字段，形如 `"minAppVersion":"0.3.0",`
fn package_with(id: &str, version: &str, extra: &str) -> Vec<u8> {
    let manifest = format!(
        r#"{{"id":"{id}","version":"{version}",{extra}"name":"i18n:name","description":"i18n:description","category":"text","functions":{{"run":{{}}}}}}"#
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

fn temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("tf-installed-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    root
}

fn temp(name: &str) -> Installed {
    Installed::new(temp_root(name), "0.2.0")
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
    // 带上包内的名称，界面显示「名称（id）」
    assert_eq!(err.params["names"]["zh-CN"], "示例");

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

#[test]
fn records_localized_names() {
    let installed = temp("names");
    let (info, _) = installed
        .install(&package("org.example.named", "1.0.0"), None, true, |_| {
            false
        })
        .unwrap();
    assert_eq!(info.names.get("zh-CN").map(String::as_str), Some("示例"));
    assert_eq!(installed.list()[0].names, info.names);
    // 未声明 i18n 的名称原样记为 "*"
    let raw = tf_package::open(&package("org.example.raw", "1.0.0")).unwrap();
    let mut plain = raw;
    plain.manifest.name = "Plain".into();
    assert_eq!(
        package_names(&plain).get("*").map(String::as_str),
        Some("Plain")
    );
    std::fs::remove_dir_all(installed.root()).unwrap();
}

#[test]
fn rejects_packages_that_need_a_newer_app() {
    let installed = temp("needs-app");
    let bytes = package_with("org.example.demo", "1.0.0", r#""minAppVersion":"0.3.0","#);
    let err = installed
        .install(&bytes, None, true, |_| false)
        .unwrap_err();
    assert_eq!(err.code, "market.needs_app");
    assert_eq!(err.params["version"], "0.3.0");
    assert!(installed.list().is_empty());
}

#[test]
fn accepts_packages_within_app_version() {
    let installed = temp("within-app");
    let bytes = package_with("org.example.demo", "1.0.0", r#""minAppVersion":"0.2.0","#);
    let (_, manifest) = installed.install(&bytes, None, true, |_| false).unwrap();
    assert_eq!(manifest.min_app_version.as_deref(), Some("0.2.0"));
}

#[test]
fn skips_loading_after_app_downgrade() {
    let root = temp_root("downgrade");
    let bytes = package_with("org.example.demo", "1.0.0", r#""minAppVersion":"0.3.0","#);
    Installed::new(root.clone(), "0.3.0")
        .install(&bytes, None, true, |_| false)
        .unwrap();
    let older = Installed::new(root, "0.2.0");
    let list = older.list();
    assert_eq!(list.len(), 1, "the package stays installed");
    assert_eq!(list[0].min_app_version.as_deref(), Some("0.3.0"));
    let err = older.load("org.example.demo").err().unwrap();
    assert_eq!(err.code, "market.needs_app");
}
