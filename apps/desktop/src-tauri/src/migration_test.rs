use super::*;
use serde_json::json;

const SEMVER: &str = "org.devlive.toolforge.semver";

#[test]
fn records_handled_ids_once() {
    let root = std::env::temp_dir().join(format!("tf-migration-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    assert!(done(&root).is_empty());
    mark(&root, SEMVER).unwrap();
    mark(&root, SEMVER).unwrap();
    // 不在迁出列表中的 id 不记录
    mark(&root, "org.example.other").unwrap();
    assert_eq!(done(&root).into_iter().collect::<Vec<_>>(), [SEMVER]);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn skips_handled_and_installed_ids() {
    let none = BTreeSet::new();
    assert_eq!(pending(&none, &none), MIGRATED);
    let ids: BTreeSet<String> = [SEMVER.to_owned()].into();
    assert!(!pending(&ids, &none).contains(&SEMVER));
    assert!(!pending(&none, &ids).contains(&SEMVER));
    assert_eq!(pending(&ids, &none).len(), MIGRATED.len() - 1);
}

#[test]
fn picks_signed_compatible_entries() {
    let index = json!({ "plugins": [
        { "id": SEMVER, "url": "https://x/semver.tfp", "sha256": "ab", "signed": true, "minAppVersion": "0.2.0" },
        { "id": "org.example.unsigned", "url": "u", "sha256": "cd", "signed": false },
    ]});
    let pending = [SEMVER, "org.example.unsigned"];
    assert_eq!(
        downloads(&index, &pending, "0.2.0"),
        [Download {
            id: SEMVER.to_owned(),
            url: "https://x/semver.tfp".to_owned(),
            sha256: "ab".to_owned(),
        }]
    );
    assert!(downloads(&index, &pending, "0.1.10").is_empty());
    assert!(downloads(&index, &[], "0.2.0").is_empty());
    assert!(downloads(&json!({}), &pending, "0.2.0").is_empty());
}

#[test]
fn compares_versions_numerically() {
    assert!(at_least("0.2.0", "0.1.10"));
    assert!(at_least("0.1.10", "0.1.9"));
    assert!(at_least("1.0", "1.0.0"));
    assert!(!at_least("0.1.9", "0.1.10"));
}
