use super::*;

fn matches(glob: &str, path: &str) -> bool {
    Glob::new(glob).unwrap().matches(path)
}

#[test]
fn star_and_question_mark_stay_within_a_directory() {
    assert!(matches("a*e.c", "ace.c"));
    assert!(matches("a*e.c", "abcde.c"));
    assert!(!matches("a*e.c", "a/e.c"));
    assert!(matches("?.c", "a.c"));
    assert!(!matches("?.c", "ab.c"));
    // 不含 / 的 glob 匹配任意层级
    assert!(matches("*.js", "lib/deep/x.js"));
}

#[test]
fn double_star_crosses_directories() {
    for path in [
        "a/z.c", "amnz.c", "am/nz.c", "a/mnz.c", "amn/z.c", "a/mn/z.c",
    ] {
        assert!(matches("a**z.c", path), "{path}");
    }
    assert!(matches("a/**/z.c", "a/z.c"));
    assert!(matches("a/**/z.c", "a/b/c/z.c"));
    assert!(!matches("a/**/z.c", "b/a/z.c"));
}

#[test]
fn globs_with_slashes_are_relative_to_the_config() {
    assert!(matches("src/*.rs", "src/main.rs"));
    assert!(!matches("src/*.rs", "src/bin/main.rs"));
    assert!(!matches("src/*.rs", "x/src/main.rs"));
    assert!(matches("/Makefile", "Makefile"));
    assert!(!matches("/Makefile", "sub/Makefile"));
}

#[test]
fn brackets() {
    assert!(matches("[ab].a", "a.a"));
    assert!(!matches("[ab].a", "c.a"));
    assert!(matches("[!ab].b", "c.b"));
    assert!(!matches("[!ab].b", "a.b"));
    assert!(matches("[a-c].d", "b.d"));
    // 不闭合的 [ 按字面
    assert!(matches("ab[e.c", "ab[e.c"));
    // 字符集中出现 / 时按字面
    assert!(matches("ab[/]ef", "ab[/]ef"));
}

#[test]
fn braces_and_numeric_ranges() {
    assert!(matches("*.{py,js}", "a.py"));
    assert!(matches("*.{py,js}", "b.js"));
    assert!(!matches("*.{py,js}", "c.rs"));
    // 嵌套备选
    assert!(matches("{a,{b,c}}.e", "c.e"));
    assert!(!matches("{a,{b,c}}.e", "d.e"));
    // 单个元素与空花括号按字面
    assert!(matches("{single}.b", "{single}.b"));
    assert!(!matches("{single}.b", "single.b"));
    assert!(matches("{}.c", "{}.c"));
    // 数字范围（含负数）
    assert!(matches("file{3..120}", "file3"));
    assert!(matches("file{3..120}", "file120"));
    assert!(!matches("file{3..120}", "file121"));
    assert!(!matches("file{3..120}", "file2"));
    assert!(matches("n{-5..5}", "n-3"));
    assert!(!matches("n{-5..5}", "n-6"));
    // 转义
    assert!(matches("\\*.c", "*.c"));
    assert!(!matches("\\*.c", "a.c"));
}
