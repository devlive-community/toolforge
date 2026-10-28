use super::*;

fn sample() -> Project {
    create("w1", "Ball", 320, 240).unwrap()
}

fn no_sizes() -> BTreeMap<String, u64> {
    BTreeMap::new()
}

#[test]
fn new_project_has_one_blank_frame() {
    let p = sample();
    assert_eq!(p.frames, vec!["f1"]);
    assert!(validate(&p).is_ok());
}

#[test]
fn duplicate_gets_a_new_id_that_points_at_its_source() {
    let edit = apply(sample(), Op::Duplicate { index: 0 }, &no_sizes()).unwrap();
    let created = edit.created.unwrap();
    assert_eq!(
        edit.project.frames,
        vec!["f1".to_string(), created.id.clone()]
    );
    assert_ne!(
        created.id, "f1",
        "复制帧与原帧编号不同，之后再画不会改到原帧"
    );
    assert_eq!(created.source.as_deref(), Some("f1"));
}

#[test]
fn replacing_content_never_reuses_the_old_id() {
    let p = apply(sample(), Op::Duplicate { index: 0 }, &no_sizes())
        .unwrap()
        .project;
    let edit = apply(p, Op::Replace { index: 1, size: 10 }, &no_sizes()).unwrap();
    assert_eq!(edit.project.frames[0], "f1");
    assert_ne!(edit.project.frames[1], "f2");
    assert!(
        edit.project.pool.contains(&"f2".to_string()),
        "旧内容留给撤销"
    );
}

#[test]
fn the_last_frame_cannot_be_deleted() {
    assert_eq!(
        apply(sample(), Op::Delete { index: 0 }, &no_sizes())
            .unwrap_err()
            .code,
        "flipbook.last_frame"
    );
}

#[test]
fn move_reorders_frames() {
    let mut p = sample();
    for i in 0..2 {
        p = apply(p, Op::Add { index: i }, &no_sizes()).unwrap().project;
    }
    let moved = apply(p, Op::Move { from: 0, to: 2 }, &no_sizes())
        .unwrap()
        .project;
    assert_eq!(moved.frames, vec!["f2", "f3", "f1"]);
}

#[test]
fn restore_only_accepts_known_frames() {
    let p = apply(sample(), Op::Add { index: 0 }, &no_sizes())
        .unwrap()
        .project;
    let undone = apply(
        p.clone(),
        Op::Restore {
            frames: vec!["f1".into()],
        },
        &no_sizes(),
    )
    .unwrap();
    assert_eq!(undone.project.frames, vec!["f1"]);
    let bad = apply(
        p,
        Op::Restore {
            frames: vec!["f99".into()],
        },
        &no_sizes(),
    );
    assert_eq!(bad.unwrap_err().code, "flipbook.bad_project");
}

#[test]
fn frame_count_is_capped() {
    let mut p = sample();
    for i in 0..MAX_FRAMES - 1 {
        p = apply(p, Op::Add { index: i }, &no_sizes()).unwrap().project;
    }
    let err = apply(p, Op::Add { index: 0 }, &no_sizes()).unwrap_err();
    assert_eq!(err.code, "flipbook.too_many_frames");
}

#[test]
fn project_capacity_is_enforced_including_the_new_content() {
    let p = sample();
    let mut sizes = BTreeMap::new();
    sizes.insert("f1".to_string(), MAX_PROJECT_TEXT - 5);
    let err = apply(p.clone(), Op::Duplicate { index: 0 }, &sizes).unwrap_err();
    assert_eq!(err.code, "flipbook.project_full");
    // 替换时旧内容不再计入
    assert!(
        apply(
            p,
            Op::Replace {
                index: 0,
                size: 100
            },
            &sizes
        )
        .is_ok()
    );
}

#[test]
fn delay_is_rounded_to_ten_ms_and_clamped() {
    let p = apply(sample(), Op::Delay { ms: 124 }, &no_sizes())
        .unwrap()
        .project;
    assert_eq!(p.delay_ms, 120);
    let p = apply(p, Op::Delay { ms: 5 }, &no_sizes()).unwrap().project;
    assert_eq!(p.delay_ms, MIN_DELAY);
    let p = apply(p, Op::Delay { ms: 99_999 }, &no_sizes())
        .unwrap()
        .project;
    assert_eq!(p.delay_ms, MAX_DELAY);
}

#[test]
fn collect_drops_frames_no_longer_referenced() {
    let p = apply(sample(), Op::Replace { index: 0, size: 1 }, &no_sizes())
        .unwrap()
        .project;
    let (kept, gone) = collect(p.clone(), &[]).unwrap();
    assert_eq!(gone, vec!["f1"]);
    assert_eq!(kept.pool, p.frames);
    // 撤销历史仍引用的旧帧保留
    let (_, gone) = collect(p, &["f1".to_string()]).unwrap();
    assert!(gone.is_empty());
}

#[test]
fn bad_names_and_ids_are_rejected() {
    assert!(create("w1", "   ", 320, 240).is_err());
    assert!(create("../x", "ok", 320, 240).is_err());
    assert!(create("w1", "ok", 333, 240).is_err());
}
