use super::*;

fn stats(input: &str) -> Stats {
    collect(Args {
        input: input.into(),
    })
}

#[test]
fn counts_mixed_chinese_and_english() {
    let s = stats("Hello world, hello ToolForge!\n\n你好，世界。👍🏽");
    assert_eq!(s.cjk_characters, 4);
    assert_eq!(s.latin_words, 4);
    assert_eq!(s.words, 8);
    assert_eq!(s.lines, 3);
    assert_eq!(s.non_empty_lines, 2);
    assert_eq!(s.paragraphs, 2);
    assert_eq!(s.top_words[0], ("hello".to_owned(), 2));
}

#[test]
fn emoji_with_modifiers_is_one_character() {
    let s = stats("👍🏽a");
    assert_eq!(s.characters, 2);
    assert_eq!(s.bytes, "👍🏽a".len());
}

#[test]
fn empty_text() {
    let s = stats("");
    assert_eq!(
        (
            s.characters,
            s.words,
            s.lines,
            s.paragraphs,
            s.reading_seconds
        ),
        (0, 0, 0, 0, 0)
    );
}

#[test]
fn reading_time_scales_with_length() {
    let text = "word ".repeat(440);
    assert_eq!(stats(&text).reading_seconds, 120);
}
