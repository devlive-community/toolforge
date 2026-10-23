use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use super::*;

/// plugins/ 下声明了 CLI 接口的内置插件必须都登记在 all() 中，反之亦然
#[test]
fn every_builtin_with_a_cli_interface_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let on_disk: BTreeSet<String> = std::fs::read_dir(&root)
        .expect("plugins directory")
        .filter_map(|entry| {
            let raw = std::fs::read_to_string(entry.ok()?.path().join("manifest.json")).ok()?;
            let manifest: Value = serde_json::from_str(&raw).expect("valid manifest");
            let cli = manifest["functions"].as_object()?.values().any(|spec| {
                spec.pointer("/interface/expose")
                    .and_then(Value::as_array)
                    .is_some_and(|c| c.iter().any(|c| c == "cli"))
            });
            cli.then(|| manifest["id"].as_str().unwrap().to_owned())
        })
        .collect();
    let registered: BTreeSet<String> = all()
        .iter()
        .map(|b| (b.create)().manifest().id.clone())
        .collect();
    assert_eq!(on_disk, registered);
}

#[test]
fn every_builtin_bundles_its_declared_locales() {
    for builtin in all() {
        let plugin = (builtin.create)();
        let manifest = plugin.manifest();
        for locale in &manifest.locales {
            let raw = builtin
                .locales
                .iter()
                .find(|(lang, _)| lang == locale)
                .unwrap_or_else(|| panic!("{} misses locale {locale}", manifest.id))
                .1;
            let messages: Value = serde_json::from_str(raw).unwrap();
            assert!(messages["name"].is_string(), "{} {locale}", manifest.id);
        }
    }
}
