use super::*;
use crate::test_support::{workspace, write};

fn options(roots: Vec<String>) -> Options {
    serde_json::from_value(serde_json::json!({ "roots": roots })).unwrap()
}

fn names(files: &[FileInfo]) -> Vec<String> {
    let mut names: Vec<String> = files
        .iter()
        .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn walks_folders_and_skips_hidden_excluded_and_empty() {
    let dir = workspace("scan");
    write(&dir, "a.txt", b"a");
    write(&dir, "sub/b.jpg", b"b");
    write(&dir, "empty.txt", b"");
    write(&dir, ".hidden", b"h");
    write(&dir, ".cache/c.txt", b"c");
    write(&dir, "node_modules/lib.js", b"x");
    let root = dir.to_string_lossy().into_owned();
    let files = scan(&options(vec![root.clone()]), |_| {}, || false).unwrap();
    assert_eq!(names(&files), ["a.txt", "b.jpg"]);

    let mut all = options(vec![root.clone()]);
    all.include_hidden = true;
    all.exclude.clear();
    all.min_size = 0;
    assert_eq!(
        names(&scan(&all, |_| {}, || false).unwrap()),
        [".hidden", "a.txt", "b.jpg", "c.txt", "lib.js"]
    );

    let mut images = options(vec![root]);
    images.extensions = vec![".JPG".into()];
    assert_eq!(names(&scan(&images, |_| {}, || false).unwrap()), ["b.jpg"]);
}

#[test]
fn ignores_nested_roots_and_rejects_missing_folders() {
    let dir = workspace("roots");
    write(&dir, "sub/a.txt", b"a");
    let roots = vec![
        dir.join("sub").to_string_lossy().into_owned(),
        dir.to_string_lossy().into_owned(),
    ];
    let files = scan(&options(roots), |_| {}, || false).unwrap();
    assert_eq!(files.len(), 1, "the nested root is not scanned twice");
    assert_eq!(
        scan(&options(vec![]), |_| {}, || false).unwrap_err().code,
        "dup.no_folders"
    );
    let missing = scan(
        &options(vec!["/definitely/missing".into()]),
        |_| {},
        || false,
    );
    assert_eq!(missing.unwrap_err().code, "dup.not_a_folder");
}

#[cfg(unix)]
#[test]
fn does_not_follow_symlinks() {
    let dir = workspace("links");
    write(&dir, "real.txt", b"data");
    std::os::unix::fs::symlink(dir.join("real.txt"), dir.join("link.txt")).unwrap();
    let files = scan(
        &options(vec![dir.to_string_lossy().into_owned()]),
        |_| {},
        || false,
    )
    .unwrap();
    assert_eq!(names(&files), ["real.txt"]);
}
