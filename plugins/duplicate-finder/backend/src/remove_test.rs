use super::*;
use crate::test_support::{Ctx, workspace, write};

fn hash(content: &[u8]) -> String {
    blake3::hash(content).to_hex().to_string()
}

fn delete(path: &Path) -> Result<(), String> {
    std::fs::remove_file(path).map_err(|e| e.to_string())
}

#[test]
fn removes_verified_copies_only() {
    let dir = workspace("remove");
    let keep = write(&dir, "keep.txt", b"content");
    let copy = write(&dir, "copy.txt", b"content");
    let changed = write(&dir, "changed.txt", b"CONTENT");
    let ctx = Ctx::default();
    let selection = Selection {
        hash: hash(b"content"),
        keep: vec![keep.clone()],
        remove: vec![
            copy.clone(),
            changed.clone(),
            dir.join("gone.txt").to_string_lossy().into_owned(),
        ],
    };
    let outcome = remove(&[selection], &ctx, &delete).unwrap();
    assert_eq!(outcome.removed, std::slice::from_ref(&copy));
    assert_eq!(outcome.freed, 7);
    assert_eq!(outcome.skipped.len(), 2);
    assert!(
        Path::new(&keep).exists() && !Path::new(&copy).exists() && Path::new(&changed).exists()
    );
}

#[test]
fn keeps_everything_when_the_kept_file_changed() {
    let dir = workspace("kept");
    let keep = write(&dir, "keep.txt", b"edited since the scan");
    let copy = write(&dir, "copy.txt", b"content");
    let selection = Selection {
        hash: hash(b"content"),
        keep: vec![keep],
        remove: vec![copy.clone()],
    };
    let outcome = remove(&[selection], &Ctx::default(), &delete).unwrap();
    assert!(outcome.removed.is_empty());
    assert!(Path::new(&copy).exists());
}

#[test]
fn requires_a_kept_file_and_reports_trash_errors() {
    let selection = Selection {
        hash: "x".into(),
        keep: vec![],
        remove: vec!["/a".into()],
    };
    assert_eq!(
        remove(&[selection], &Ctx::default(), &delete)
            .unwrap_err()
            .code,
        "dup.keep_one"
    );

    let dir = workspace("fail");
    let keep = write(&dir, "keep.txt", b"content");
    let copy = write(&dir, "copy.txt", b"content");
    let ctx = Ctx::default();
    let selection = Selection {
        hash: hash(b"content"),
        keep: vec![keep],
        remove: vec![copy.clone()],
    };
    let outcome = remove(&[selection], &ctx, &|_| Err("denied".into())).unwrap();
    assert_eq!(outcome.skipped, [copy]);
    assert!(
        ctx.logs
            .lock()
            .unwrap()
            .iter()
            .any(|(code, _)| code == "dup.trash_failed")
    );
}
