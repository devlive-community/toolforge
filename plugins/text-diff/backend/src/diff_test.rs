use super::*;

fn args(left: &str, right: &str) -> Args {
    Args {
        left: left.into(),
        right: right.into(),
        mode: Mode::Lines,
        algorithm: Algo::Myers,
        ignore_case: false,
        ignore_whitespace: false,
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
