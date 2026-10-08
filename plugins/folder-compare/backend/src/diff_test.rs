use super::*;
use crate::test_support::{dir, write};

#[test]
fn pairs_deletions_and_insertions_side_by_side() {
    let root = dir("diff");
    write(&root, "a.txt", "one\ntwo\nthree\nfour\n");
    write(&root, "b.txt", "one\nTWO\nthree\nfour\nfive\n");
    let out = diff(&root.join("a.txt"), &root.join("b.txt")).unwrap();
    let tags: Vec<Tag> = out.rows.iter().map(|r| r.tag).collect();
    assert_eq!(
        tags,
        [Tag::Equal, Tag::Change, Tag::Equal, Tag::Equal, Tag::Insert]
    );
    assert_eq!(out.rows[1].left.as_ref().unwrap().text, "two");
    assert_eq!(out.rows[1].right.as_ref().unwrap().text, "TWO");
    assert_eq!(out.rows[4].right.as_ref().unwrap().number, 5);
    assert_eq!((out.added, out.removed), (2, 1));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn treats_a_missing_side_as_empty_and_rejects_binary() {
    let root = dir("diff-edge");
    write(&root, "only.txt", "a\r\nb\r\n");
    let out = diff(&root.join("only.txt"), &root.join("missing.txt")).unwrap();
    assert!(out.rows.iter().all(|r| r.tag == Tag::Delete));
    assert_eq!(out.rows[0].left.as_ref().unwrap().text, "a");
    std::fs::write(root.join("bin"), b"\0\x01\x02").unwrap();
    assert_eq!(
        diff(&root.join("bin"), &root.join("only.txt"))
            .unwrap_err()
            .code,
        "fc.binary"
    );
    std::fs::remove_dir_all(root).unwrap();
}
