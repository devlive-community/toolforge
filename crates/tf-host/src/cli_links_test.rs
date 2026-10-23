use super::*;

fn temp(name: &str) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("tf-cli-links-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let sidecar = root.join(if cfg!(windows) { "tfp.exe" } else { "tfp" });
    std::fs::write(&sidecar, b"binary").unwrap();
    (root.join("bin"), sidecar)
}

#[test]
fn writes_entries_and_prunes_stale_ones() {
    let (dir, sidecar) = temp("write");
    assert!(!installed(&dir));
    write(&dir, &sidecar, &["hash".to_owned(), "old".to_owned()]).unwrap();
    assert!(installed(&dir));
    assert_eq!(commands(&dir), ["tfp-hash", "tfp-old"]);

    std::fs::write(dir.join("notes.txt"), b"keep").unwrap();
    write(
        &dir,
        &sidecar,
        &["hash".to_owned(), "json-formatter".to_owned()],
    )
    .unwrap();
    assert_eq!(commands(&dir), ["tfp-hash", "tfp-json-formatter"]);
    assert!(
        dir.join("notes.txt").exists(),
        "files we did not create are kept"
    );

    let entry = dir.join(entry_name("tfp-hash"));
    if cfg!(windows) {
        let script = std::fs::read_to_string(entry).unwrap();
        assert!(script.contains(" hash %*"));
    } else {
        assert_eq!(std::fs::read_link(entry).unwrap(), sidecar);
    }
    std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
}

#[test]
fn remove_deletes_only_our_entries() {
    let (dir, sidecar) = temp("remove");
    write(&dir, &sidecar, &["hash".to_owned()]).unwrap();
    remove(&dir).unwrap();
    assert!(!dir.exists(), "an emptied directory is removed");

    write(&dir, &sidecar, &["hash".to_owned()]).unwrap();
    std::fs::write(dir.join("mine"), b"x").unwrap();
    remove(&dir).unwrap();
    assert!(!installed(&dir));
    assert!(dir.join("mine").exists());
    remove(&dir.join("missing")).unwrap();
    std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
}

#[test]
fn sidecar_sits_beside_the_app() {
    let exe = Path::new("/Applications/ToolForge.app/Contents/MacOS/toolforge");
    let expected = if cfg!(windows) { "tfp.exe" } else { "tfp" };
    assert_eq!(sidecar_beside(exe), exe.with_file_name(expected));
}
