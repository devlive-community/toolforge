use super::*;
use crate::scan::{Ignore, scan};
use crate::test_support::{dir, touch, write};

fn statuses(items: &[Item]) -> Vec<(String, Status)> {
    items.iter().map(|i| (i.path.clone(), i.status)).collect()
}

fn fixture() -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let base = dir("compare");
    let (l, r) = (base.join("l"), base.join("r"));
    for (root, extra) in [(&l, "left"), (&r, "right")] {
        write(root, "same.txt", "same");
        write(root, "docs/guide.md", "guide");
        write(root, &format!("only-{extra}.txt"), extra);
        touch(root, "same.txt", 1_000);
        touch(root, "docs/guide.md", 1_000);
    }
    write(&l, "edited.txt", "old!");
    write(&r, "edited.txt", "new!");
    touch(&l, "edited.txt", 1_000);
    touch(&r, "edited.txt", 1_000);
    write(&l, "grown.txt", "a");
    write(&r, "grown.txt", "abc");
    write(&l, "mixed", "file");
    write(&r, "mixed/inner.txt", "dir");
    write(&r, "docs/new.md", "new");
    (base, l, r)
}

fn run(l: &std::path::Path, r: &std::path::Path, method: Method) -> (Vec<Item>, Summary) {
    let ignore = Ignore::new(&[], false);
    let (a, b) = (
        scan(l, &ignore, &|| false).unwrap(),
        scan(r, &ignore, &|| false).unwrap(),
    );
    let same = if method == Method::Content {
        same_contents(l, r, &candidates(&a.entries, &b.entries), &|_| {}, &|| {
            false
        })
    } else {
        Default::default()
    };
    merge(&a.entries, &b.entries, method, &same)
}

#[test]
fn quick_mode_trusts_size_and_time() {
    let (base, l, r) = fixture();
    let (items, summary) = run(&l, &r, Method::Quick);
    assert_eq!(
        statuses(&items),
        [
            ("docs".to_owned(), Status::Changed),
            ("docs/guide.md".to_owned(), Status::Same),
            ("docs/new.md".to_owned(), Status::RightOnly),
            ("mixed".to_owned(), Status::Mismatch),
            ("mixed/inner.txt".to_owned(), Status::RightOnly),
            ("edited.txt".to_owned(), Status::Same),
            ("grown.txt".to_owned(), Status::Changed),
            ("only-left.txt".to_owned(), Status::LeftOnly),
            ("only-right.txt".to_owned(), Status::RightOnly),
            ("same.txt".to_owned(), Status::Same),
        ]
    );
    // 同大小同时间但内容不同，快速模式发现不了
    assert_eq!(summary.same, 3);
    assert_eq!(summary.mismatch, 1);
    let mixed = items.iter().find(|i| i.path == "mixed").unwrap();
    assert!(mixed.dir);
    assert!(!mixed.left.as_ref().unwrap().dir);
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn content_mode_reads_files_of_equal_size() {
    let (base, l, r) = fixture();
    let (items, summary) = run(&l, &r, Method::Content);
    let edited = items.iter().find(|i| i.path == "edited.txt").unwrap();
    assert_eq!(edited.status, Status::Changed);
    assert_eq!(edited.newer, None, "same modification time");
    assert_eq!(summary.same, 2);
    assert_eq!(summary.changed, 2);
    assert_eq!(summary.left_only, 1);
    assert_eq!(summary.right_only, 3);
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn reports_the_newer_side() {
    let (base, l, r) = fixture();
    touch(&l, "grown.txt", 5_000);
    touch(&r, "grown.txt", 9_000);
    let (items, _) = run(&l, &r, Method::Quick);
    let grown = items.iter().find(|i| i.path == "grown.txt").unwrap();
    assert_eq!(grown.newer, Some(Side::Right));
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn identical_trees_are_all_same() {
    let base = dir("identical");
    let (l, r) = (base.join("l"), base.join("r"));
    for root in [&l, &r] {
        write(root, "a/b/c.txt", "x");
        touch(root, "a/b/c.txt", 1_000);
    }
    let (items, _) = run(&l, &r, Method::Quick);
    assert!(items.iter().all(|i| i.status == Status::Same));
    std::fs::remove_dir_all(base).unwrap();
}
