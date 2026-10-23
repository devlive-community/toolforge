use super::*;

fn flags_of(text: &str) -> Vec<(String, Vec<Flag>)> {
    analyze(text)
        .chars
        .into_iter()
        .filter(|c| !c.flags.is_empty())
        .map(|c| (c.code, c.flags))
        .collect()
}

#[test]
fn describes_characters() {
    let result = analyze("A中😀\n");
    let a = &result.chars[0];
    assert_eq!(
        (
            a.code.as_str(),
            a.name.as_deref(),
            a.category,
            a.script.as_str()
        ),
        ("U+0041", Some("LATIN CAPITAL LETTER A"), "Lu", "Latin")
    );
    let han = &result.chars[1];
    assert_eq!(
        (han.utf8.as_str(), han.utf16.as_str(), han.name.as_deref()),
        ("E4 B8 AD", "4E2D", Some("CJK UNIFIED IDEOGRAPH-4E2D"))
    );
    let emoji = &result.chars[2];
    assert_eq!(
        (emoji.utf8.as_str(), emoji.utf16.as_str()),
        ("F0 9F 98 80", "D83D DE00")
    );
    assert_eq!(result.chars[3].offset, 4, "offsets count UTF-16 units");
    assert_eq!(result.chars[3].name.as_deref(), Some("LINE FEED (LF)"));
    let counts = &result.counts;
    assert_eq!(
        (
            counts.chars,
            counts.graphemes,
            counts.utf8_bytes,
            counts.utf16_units,
            counts.lines
        ),
        (4, 4, 9, 5, 1)
    );
}

#[test]
fn counts_graphemes_separately() {
    // 家庭 emoji 由 7 个码位组成，是一个字形
    let family = "👨‍👩‍👧‍👦";
    let counts = analyze(family).counts;
    assert_eq!((counts.chars, counts.graphemes), (7, 1));
    assert_eq!(counts.invisible, 0, "joiners inside emoji are not reported");
    // 单独出现的零宽连接符仍然提示
    assert_eq!(analyze("a\u{200d}b").counts.invisible, 1);
    assert_eq!(analyze("❤\u{fe0f}").counts.invisible, 0);
    let accented = "e\u{301}";
    let result = analyze(accented);
    assert_eq!(result.counts.graphemes, 1);
    assert_eq!(result.chars[1].flags, vec![Flag::Combining]);
    assert!(!result.normalization.nfc && result.normalization.nfd);
}

#[test]
fn flags_hidden_and_dangerous_characters() {
    let text = "pass\u{200b}word \u{202e}gnp.exe\u{202c} caf\u{00e9}\u{00a0}x \u{feff}\u{0007}\t";
    let flagged = flags_of(text);
    let codes: Vec<&str> = flagged.iter().map(|(c, _)| c.as_str()).collect();
    assert_eq!(
        codes,
        ["U+200B", "U+202E", "U+202C", "U+00A0", "U+FEFF", "U+0007"]
    );
    assert!(flagged[1].1.contains(&Flag::Bidi));
    assert_eq!(flagged[3].1, vec![Flag::Space]);
    assert_eq!(flagged[5].1, vec![Flag::Control]);
    let counts = analyze(text).counts;
    assert_eq!(
        (counts.invisible, counts.bidi, counts.space, counts.control),
        (2, 2, 1, 1)
    );
}

#[test]
fn finds_confusables_and_mixed_scripts() {
    // 西里尔字母 а、о 与拉丁字母外形相同
    let text = "p\u{0430}yp\u{0430}l.com g\u{043e}\u{043e}gle 日本語テキスト 한국어漢字";
    let result = analyze(text);
    let confusable: Vec<(&str, Option<&str>)> = result
        .chars
        .iter()
        .filter(|c| c.flags.contains(&Flag::Confusable))
        .map(|c| (c.code.as_str(), c.lookalike.as_deref()))
        .collect();
    assert_eq!(confusable[0], ("U+0430", Some("a")));
    assert_eq!(result.counts.confusable, 4);
    assert_eq!(result.mixed_words, ["pаypаl", "gооgle"]);
    assert!(
        analyze("ＡＢＣ").chars[0].flags.contains(&Flag::Confusable),
        "fullwidth letters look like ASCII"
    );
    assert!(
        analyze("plain ascii")
            .chars
            .iter()
            .all(|c| c.flags.is_empty())
    );
}

#[test]
fn cleans_text() {
    let text = "a\u{200b}b\u{202e}c\u{00a0}d p\u{0430}y ﬁ 👨‍👩‍👧";
    let options = |replace: bool, form: Form| CleanOptions {
        remove_invisible: true,
        remove_bidi: true,
        normalize_spaces: true,
        replace_confusables: replace,
        remove_controls: false,
        form,
    };
    let cleaned = clean(text, &options(false, Form::None));
    assert_eq!(cleaned.text, "abc d p\u{0430}y ﬁ 👨‍👩‍👧");
    assert_eq!((cleaned.removed, cleaned.replaced), (2, 1));
    let strict = clean(text, &options(true, Form::Nfkc));
    assert_eq!(strict.text, "abc d pay fi 👨‍👩‍👧", "emoji joiners are kept");
}

#[test]
fn caps_the_character_list() {
    let long = "x".repeat(MAX_CHARS + 10);
    let result = analyze(&long);
    assert!(result.truncated);
    assert_eq!(result.chars.len(), MAX_CHARS);
    assert_eq!(result.counts.chars, MAX_CHARS + 10);
    assert_eq!(analyze("").counts.lines, 0);
}
