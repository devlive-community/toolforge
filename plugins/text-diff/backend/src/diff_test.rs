use super::*;

fn args(left: &str, right: &str) -> Args {
    Args {
        left: left.into(),
        right: right.into(),
        mode: Mode::Lines,
        algorithm: Algo::Myers,
        ignore_case: false,
        ignore_whitespace: false,
        ignore_patterns: Vec::new(),
    }
}

fn tags(out: &Output) -> Vec<Tag> {
    out.rows.iter().map(|r| r.tag).collect()
}

#[test]
fn identical_texts() {
    let out = run(args("a\nb", "a\nb")).unwrap();
    assert!(out.identical);
    assert_eq!(out.stats.similarity, 1.0);
    assert_eq!(out.unified, "");
}

#[test]
fn side_by_side_rows_pair_changes_and_number_lines() {
    let out = run(args(
        "keep\nold value\nremove me\nend",
        "keep\nnew value\nend\nadded",
    ))
    .unwrap();
    assert_eq!(
        tags(&out),
        vec![
            Tag::Equal,
            Tag::Change,
            Tag::Delete,
            Tag::Equal,
            Tag::Insert
        ]
    );
    let change = &out.rows[1];
    assert_eq!(change.left.as_ref().unwrap().number, 2);
    let right = change.right.as_ref().unwrap();
    assert_eq!(
        right.segments[0],
        Segment {
            text: "new".into(),
            emphasized: true
        }
    );
    assert_eq!(
        right.segments[1],
        Segment {
            text: " value".into(),
            emphasized: false
        }
    );
    assert_eq!(out.rows[4].right.as_ref().unwrap().number, 4);
    assert_eq!(
        (
            out.stats.added,
            out.stats.removed,
            out.stats.changed,
            out.stats.unchanged
        ),
        (1, 1, 1, 2)
    );
}

#[test]
fn ignore_case_and_whitespace_keep_original_text() {
    let mut a = args("Hello   World\n  foo", "hello world\nfoo  ");
    assert!(
        !run(Args {
            ..args("Hello   World\n  foo", "hello world\nfoo  ")
        })
        .unwrap()
        .identical
    );
    a.ignore_case = true;
    a.ignore_whitespace = true;
    let out = run(a).unwrap();
    assert!(out.identical);
    assert_eq!(
        out.rows[0].left.as_ref().unwrap().segments[0].text,
        "Hello   World"
    );
}

#[test]
fn word_and_char_modes_produce_tokens() {
    let mut words = args("the quick fox", "the slow fox");
    words.mode = Mode::Words;
    let out = run(words).unwrap();
    assert!(out.rows.is_empty());
    assert_eq!(
        out.tokens
            .iter()
            .filter(|t| t.tag != Tag::Equal)
            .map(|t| (t.tag, t.text.as_str()))
            .collect::<Vec<_>>(),
        vec![(Tag::Delete, "quick"), (Tag::Insert, "slow")]
    );
    let mut chars = args("color", "colour");
    chars.mode = Mode::Chars;
    let out = run(chars).unwrap();
    assert_eq!(out.stats.added, 1);
}

#[test]
fn unified_patch_has_headers_and_hunks() {
    let out = run(args("a\nb\nc\n", "a\nB\nc\n")).unwrap();
    assert!(out.unified.starts_with("--- left\n+++ right\n@@"));
    assert!(out.unified.contains("-b\n+B\n"));
}

#[test]
fn handles_empty_sides_and_crlf() {
    let out = run(args("", "x\r\ny")).unwrap();
    assert_eq!(tags(&out), vec![Tag::Insert, Tag::Insert]);
    assert_eq!(out.rows[0].right.as_ref().unwrap().segments[0].text, "x");
    assert!(run(args("", "")).unwrap().identical);
}

#[test]
fn ignore_patterns_normalize_noise_before_diffing() {
    let left = "2026-10-01 08:00:01 [42] start\n2026-10-01 08:00:02 [43] load config\n2026-10-01 08:00:03 [44] ready";
    let right = "2026-10-15 21:11:09 [97] start\n2026-10-15 21:11:10 [98] load config v2\n2026-10-15 21:11:11 [99] ready";
    let plain = run(args(left, right)).unwrap();
    assert_eq!(plain.stats.changed, 3);

    let mut a = args(left, right);
    a.ignore_patterns = vec![
        r"^\d{4}-\d{2}-\d{2} [\d:]+".into(),
        r"\[\d+\]".into(),
        String::new(),
    ];
    let out = run(a).unwrap();
    assert_eq!(tags(&out), vec![Tag::Equal, Tag::Change, Tag::Equal]);
    assert_eq!(
        (out.stats.unchanged, out.stats.changed, out.stats.ignored),
        (2, 1, 2)
    );
    // 显示原文
    assert!(
        out.rows[0].left.as_ref().unwrap().segments[0]
            .text
            .starts_with("2026-10-01")
    );
}

#[test]
fn ignore_patterns_stack_with_case_and_whitespace() {
    let mut a = args("ID=1  Hello", "id=2 hello");
    a.ignore_patterns = vec![r"(?i)id=\d+".into()];
    assert!(
        !run(Args {
            ignore_patterns: a.ignore_patterns.clone(),
            ..args("ID=1  Hello", "id=2 hello")
        })
        .unwrap()
        .identical
    );
    a.ignore_case = true;
    a.ignore_whitespace = true;
    assert!(run(a).unwrap().identical);
}

#[test]
fn rejects_invalid_patterns() {
    let mut a = args("a", "b");
    a.ignore_patterns = vec!["ok".into(), "(".into()];
    let err = run(a).unwrap_err();
    assert_eq!(err.code, "diff.invalid_pattern");
    assert_eq!(err.params["index"], 2);
}
