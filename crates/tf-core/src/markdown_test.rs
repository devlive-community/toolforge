use super::*;

fn text(text: &str) -> Span {
    Span {
        text: text.into(),
        ..Span::default()
    }
}

#[test]
fn parses_release_notes() {
    let blocks = parse(
        "## What's Changed\n\n### Features\n\n- **encoder**: add file mode (`abc1234`)\n- plain item\n\n**Full Changelog**: https://github.com/a/b/compare/v1...v2",
    );
    assert_eq!(blocks.len(), 4);
    assert_eq!(
        blocks[0],
        Block::Heading {
            level: 2,
            spans: vec![text("What's Changed")]
        }
    );
    let Block::List { ordered, items, .. } = &blocks[2] else {
        panic!("expected list: {blocks:?}");
    };
    assert!(!ordered);
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0],
        vec![Block::Paragraph {
            spans: vec![
                Span {
                    strong: true,
                    ..text("encoder")
                },
                text(": add file mode ("),
                Span {
                    code: true,
                    ..text("abc1234")
                },
                text(")"),
            ]
        }]
    );
    let Block::Paragraph { spans } = &blocks[3] else {
        panic!("expected paragraph");
    };
    let link = spans.last().unwrap();
    assert_eq!(
        link.href.as_deref(),
        Some("https://github.com/a/b/compare/v1...v2")
    );
    assert_eq!(link.text, "https://github.com/a/b/compare/v1...v2");
}

#[test]
fn keeps_only_http_links_and_drops_html() {
    let blocks = parse("[a](javascript:alert(1)) [b](https://x.dev) <script>x</script>");
    let Block::Paragraph { spans } = &blocks[0] else {
        panic!("expected paragraph");
    };
    assert_eq!(spans[0].href, None);
    assert_eq!(spans[0].text, "a ");
    assert_eq!(spans[1].href.as_deref(), Some("https://x.dev"));
    assert!(spans.iter().all(|s| !s.text.contains("script")));
}

#[test]
fn autolink_trims_trailing_punctuation() {
    let spans = autolink(text("see https://x.dev/a. done"));
    assert_eq!(spans.len(), 3);
    assert_eq!(spans[1].href.as_deref(), Some("https://x.dev/a"));
    assert_eq!(spans[2].text, ". done");
}

#[test]
fn nested_lists_quotes_and_code() {
    let blocks = parse("1. one\n   - nested\n2. two\n\n> quoted\n\n```\nlet a = 1;\n```\n\n---");
    let Block::List {
        ordered,
        start,
        items,
    } = &blocks[0]
    else {
        panic!("expected list");
    };
    assert!(ordered);
    assert_eq!(*start, 1);
    assert!(matches!(items[0][1], Block::List { ordered: false, .. }));
    assert_eq!(
        blocks[1],
        Block::Quote {
            blocks: vec![Block::Paragraph {
                spans: vec![text("quoted")]
            }]
        }
    );
    assert_eq!(
        blocks[2],
        Block::Code {
            text: "let a = 1;".into()
        }
    );
    assert_eq!(blocks[3], Block::Rule);
}

#[test]
fn empty_input_has_no_blocks() {
    assert!(parse("").is_empty());
    assert!(parse("   \n\n").is_empty());
}
