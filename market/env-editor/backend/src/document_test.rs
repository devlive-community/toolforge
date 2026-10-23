use super::*;

#[test]
fn parses_plain_values() {
    let lines = parse("KEY=value\n");
    assert_eq!(
        lines,
        vec![Line::Entry {
            r#export: false,
            key: "KEY".into(),
            value: "value".into(),
            quote: Quote::None,
            inline_comment: None,
            multiline: false,
        }]
    );
}

#[test]
fn keeps_comments_blanks_and_order() {
    let lines = parse("# header\n\nA=1\nB=2\n");
    assert_eq!(
        lines,
        vec![
            Line::Comment {
                text: "# header".into()
            },
            Line::Blank,
            Line::Entry {
                r#export: false,
                key: "A".into(),
                value: "1".into(),
                quote: Quote::None,
                inline_comment: None,
                multiline: false
            },
            Line::Entry {
                r#export: false,
                key: "B".into(),
                value: "2".into(),
                quote: Quote::None,
                inline_comment: None,
                multiline: false
            },
        ]
    );
}

#[test]
fn reads_quoted_values_and_export_prefix() {
    let lines = parse("S='single value'\nexport DB=\"postgres://localhost\\\"5432\"\n");
    assert_eq!(
        lines[0],
        Line::Entry {
            r#export: false,
            key: "S".into(),
            value: "single value".into(),
            quote: Quote::Single,
            inline_comment: None,
            multiline: false,
        }
    );
    assert_eq!(
        lines[1],
        Line::Entry {
            r#export: true,
            key: "DB".into(),
            value: "postgres://localhost\"5432".into(),
            quote: Quote::Double,
            inline_comment: None,
            multiline: false,
        }
    );
}

#[test]
fn splits_inline_comments_only_after_whitespace_hash() {
    let lines = parse("URL=https://example.com#anchor\nNAME=value # inline\nEMPTY= # note\n");
    assert_eq!(
        lines[0],
        Line::Entry {
            r#export: false,
            key: "URL".into(),
            value: "https://example.com#anchor".into(),
            quote: Quote::None,
            inline_comment: None,
            multiline: false
        }
    );
    assert_eq!(
        lines[1].clone(),
        Line::Entry {
            r#export: false,
            key: "NAME".into(),
            value: "value".into(),
            quote: Quote::None,
            inline_comment: Some("inline".into()),
            multiline: false
        }
    );
    assert_eq!(
        lines[2].clone(),
        Line::Entry {
            r#export: false,
            key: "EMPTY".into(),
            value: String::new(),
            quote: Quote::None,
            inline_comment: Some("note".into()),
            multiline: false
        }
    );
}

#[test]
fn parses_multiline_double_quoted_blocks() {
    let text = "SCRIPT=\"echo a\necho b\nexit 0\" # setup\nAFTER=1\n";
    let lines = parse(text);
    assert_eq!(
        lines[0],
        Line::Entry {
            r#export: false,
            key: "SCRIPT".into(),
            value: "echo a\necho b\nexit 0".into(),
            quote: Quote::Double,
            inline_comment: Some("setup".into()),
            multiline: true,
        }
    );
    assert_eq!(
        lines[1],
        Line::Entry {
            r#export: false,
            key: "AFTER".into(),
            value: "1".into(),
            quote: Quote::None,
            inline_comment: None,
            multiline: false
        }
    );
}

#[test]
fn passes_through_unterminated_quotes_and_bad_lines() {
    let lines = parse("OK=1\nBROKEN=\"never closes\nTAIL=2\n= novalue\n1BAD=x\njust text\n");
    assert_eq!(
        lines[0],
        Line::Entry {
            r#export: false,
            key: "OK".into(),
            value: "1".into(),
            quote: Quote::None,
            inline_comment: None,
            multiline: false
        }
    );
    // 未闭合双引号：到文件末尾整体透传
    assert_eq!(
        lines[1],
        Line::Passthrough {
            text: "BROKEN=\"never closes\nTAIL=2\n= novalue\n1BAD=x\njust text".into()
        }
    );
}

#[test]
fn unterminated_single_quote_is_single_line_passthrough() {
    let lines = parse("A='open\nB=2\n");
    assert_eq!(
        lines[0],
        Line::Passthrough {
            text: "A='open".into()
        }
    );
    assert_eq!(
        lines[1],
        Line::Entry {
            r#export: false,
            key: "B".into(),
            value: "2".into(),
            quote: Quote::None,
            inline_comment: None,
            multiline: false
        }
    );
}

#[test]
fn round_trips_a_typical_document() {
    let text = "# service config\n\nHOST=localhost\nexport PORT=8080\nFLAGS=\"--verbose \\\\n extra\"\nSECRET='p@ss\"w0rd'\nPATH_NOQUOTE=a/b:c\nEMPTY=\n# footer\n";
    assert_eq!(serialize(&parse(text)), text);
}

#[test]
fn round_trips_display_escaping() {
    // 解析 → 显示转义 → 还原 应恒等
    let original = "line1\\nline2 \"quoted\" end\\";
    let display = escape_display(original);
    assert_eq!(unescape_double(&display), original);
}

#[test]
fn upgrades_quotes_when_serializing_risky_values() {
    let lines = vec![
        Line::Entry {
            r#export: false,
            key: "A".into(),
            value: "has space".into(),
            quote: Quote::None,
            inline_comment: None,
            multiline: false,
        },
        Line::Entry {
            r#export: false,
            key: "B".into(),
            value: "has'quote".into(),
            quote: Quote::Single,
            inline_comment: None,
            multiline: false,
        },
        Line::Entry {
            r#export: false,
            key: "C".into(),
            value: String::new(),
            quote: Quote::None,
            inline_comment: None,
            multiline: false,
        },
    ];
    assert_eq!(serialize(&lines), "A=\"has space\"\nB=\"has'quote\"\nC=\n");
}

#[test]
fn example_clears_values_but_keeps_structure() {
    let lines = parse("# db\nHOST=localhost\nexport PORT=\"5432\"\nTOKEN=abc # rotate\n");
    assert_eq!(
        to_example_text(&lines),
        "# db\nHOST=\nexport PORT=\nTOKEN= # rotate\n"
    );
}

#[test]
fn validates_keys_and_duplicates() {
    let ok = parse("A=1\nB=2\n");
    assert!(validate(&ok).is_ok());

    let dotted = parse("spring.level.root=INFO\n");
    assert!(validate(&dotted).is_ok());

    // 坏 key 在解析时就被透传，validate 只拦 UI 传入的 entry
    let bad = vec![Line::Entry {
        export: false,
        key: "1BAD".into(),
        value: "x".into(),
        quote: Quote::None,
        inline_comment: None,
        multiline: false,
    }];
    assert_eq!(validate(&bad).unwrap_err().code, "env.invalid_key");

    let dup = parse("A=1\nA=2\n");
    assert_eq!(validate(&dup).unwrap_err().code, "env.duplicate_key");
}
