use super::*;
use crate::test_support::{dir, write};

#[test]
fn matches_simple_globs() {
    assert!(glob("*.log", "server.LOG"));
    assert!(glob("node_modules", "node_modules"));
    assert!(glob("a?c", "abc"));
    assert!(glob("build/*.map", "build/app.js.map"));
    assert!(!glob("*.log", "log.txt"));
    assert!(glob("*", ""));
}

#[test]
fn ignores_names_paths_and_hidden_entries() {
    let ignore = Ignore::new(&["*.log".into(), "dist/".into(), "docs/draft".into()], true);
    assert!(ignore.matches("a/b/x.log"));
    assert!(ignore.matches("dist"));
    assert!(ignore.matches("pkg/dist"));
    assert!(ignore.matches("docs/draft"));
    assert!(!ignore.matches("other/docs/draft"));
    assert!(ignore.matches("src/.env"));
    assert!(!ignore.matches("src/main.rs"));
}

#[test]
fn scans_with_relative_slash_paths() {
    let root = dir("scan");
    write(&root, "a.txt", "aa");
    write(&root, "sub/deep/b.txt", "b");
    write(&root, "node_modules/x.js", "x");
    let ignore = Ignore::new(&["node_modules".into()], false);
    let scanned = scan(&root, &ignore, &|| false).unwrap();
    let paths: Vec<&str> = scanned.entries.keys().map(String::as_str).collect();
    assert_eq!(paths, ["a.txt", "sub", "sub/deep", "sub/deep/b.txt"]);
    assert_eq!(scanned.entries["a.txt"].size, 2);
    assert_eq!(scanned.entries["sub"].kind, Kind::Dir);
    assert!(scan(&root, &ignore, &|| true).is_none());
    std::fs::remove_dir_all(root).unwrap();
}
