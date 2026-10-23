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

    let regex = registry.get("org.devlive.toolforge.regex").unwrap();
    assert!(
        interfaces(regex.manifest()).is_empty(),
        "undeclared plugins expose nothing"
    );
}

#[test]
fn channels_reflect_declarations() {
    let registry = crate::plugins::builtin();
    let hash = registry.get("org.devlive.toolforge.hash").unwrap();
    assert_eq!(channels(hash.manifest()), ["cli", "agent"]);
    let regex = registry.get("org.devlive.toolforge.regex").unwrap();
    assert!(channels(regex.manifest()).is_empty());
}
