use super::*;

#[test]
fn lists_declared_interfaces_with_their_channels() {
    let registry = crate::plugins::builtin();
    let hash = registry.get("org.devlive.toolforge.hash").unwrap();
    let infos = interfaces(hash.manifest());
    let files = infos.iter().find(|i| i.function == "hash_files").unwrap();
    assert_eq!(files.channels, ["cli", "agent"]);
    assert_eq!(files.effect, "read");
    assert_eq!(files.uses, [FS_PERMISSION]);
    let create = infos
        .iter()
        .find(|i| i.function == "manifest_create")
        .unwrap();
    assert_eq!(create.effect, "write");

    let uuid = registry.get("org.devlive.toolforge.uuid").unwrap();
    assert!(
        interfaces(uuid.manifest()).is_empty(),
        "undeclared plugins expose nothing"
    );
}
