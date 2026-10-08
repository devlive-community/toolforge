use super::*;
use crate::test_support::{dir, touch, write};

#[test]
fn rejects_paths_that_leave_the_root() {
    let root = Path::new("/base");
    assert!(resolve(root, "a/b.txt").is_ok());
    for bad in ["", "../x", "a/../../x", "/etc/passwd"] {
        assert_eq!(
            resolve(root, bad).unwrap_err().code,
            "fc.invalid_path",
            "{bad}"
        );
    }
}

#[test]
fn copies_files_and_folders_keeping_times() {
    let base = dir("copy");
    let (l, r) = (base.join("l"), base.join("r"));
    write(&l, "a.txt", "new");
    touch(&l, "a.txt", 7_000);
    write(&r, "a.txt", "old content");
    write(&l, "pkg/x.txt", "x");
    write(&l, "pkg/sub/y.txt", "y");
    write(&l, "pkg/debug.log", "skip me");
    std::fs::create_dir_all(&r).unwrap();
    let ignore = Ignore::new(&["*.log".into()], false);
    let mut seen = Vec::new();
    let one = copy(
        &l,
        &r,
        "a.txt",
        &ignore,
        &mut |p| seen.push(p.to_owned()),
        &|| false,
    )
    .unwrap();
    assert_eq!((one.files, one.bytes), (1, 3));
    assert_eq!(std::fs::read_to_string(r.join("a.txt")).unwrap(), "new");
    let modified = std::fs::metadata(r.join("a.txt"))
        .unwrap()
        .modified()
        .unwrap();
    assert_eq!(
        modified,
        std::time::UNIX_EPOCH + std::time::Duration::from_secs(7_000)
    );
    let tree = copy(
        &l,
        &r,
        "pkg",
        &ignore,
        &mut |p| seen.push(p.to_owned()),
        &|| false,
    )
    .unwrap();
    assert_eq!(tree.files, 2);
    assert!(r.join("pkg").join("sub").join("y.txt").is_file());
    assert!(!r.join("pkg").join("debug.log").exists());
    seen.sort();
    assert_eq!(seen, ["a.txt", "pkg/sub/y.txt", "pkg/x.txt"]);
    assert!(!r.join(".a.txt.tf-part").exists());
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn refuses_to_replace_a_folder_with_a_file() {
    let base = dir("copy-mismatch");
    let (l, r) = (base.join("l"), base.join("r"));
    write(&l, "mixed", "file");
    write(&r, "mixed/inner.txt", "dir");
    let ignore = Ignore::new(&[], false);
    let err = copy(&l, &r, "mixed", &ignore, &mut |_| {}, &|| false).unwrap_err();
    assert_eq!(err.code, "fc.kind_mismatch");
    assert!(r.join("mixed").join("inner.txt").is_file());
    std::fs::remove_dir_all(base).unwrap();
}
