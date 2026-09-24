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
