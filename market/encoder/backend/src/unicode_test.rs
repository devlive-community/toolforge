use super::*;

#[test]
fn escapes_non_ascii_with_surrogate_pairs() {
    assert_eq!(escape("中😀a", Style::U, false), "\\u4e2d\\ud83d\\ude00a");
    assert_eq!(escape("a", Style::U, true), "\\u0061");
}

#[test]
fn escapes_with_braces_and_codepoints() {
    assert_eq!(
        escape("中😀a", Style::Braces, false),
        "\\u{4e2d}\\u{1f600}a"
    );
    assert_eq!(
        escape("中😀a", Style::Codepoint, false),
        "U+4E2D U+1F600 U+0061"
    );
}

#[test]
fn unescapes_every_supported_form() {
    let input = "\\u4e2d\\ud83d\\ude00\\u{1F600}\\x41\\U0001F600 plain";
    assert_eq!(unescape(input).unwrap(), "中😀😀A😀 plain");
    assert_eq!(unescape("U+4E2D U+1F600 U+0061").unwrap(), "中😀a");
}

#[test]
fn roundtrips_all_styles() {
    let text = "Tool 工具 🔧!";
    for style in [Style::U, Style::Braces, Style::Codepoint] {
        assert_eq!(
            unescape(&escape(text, style, false)).unwrap(),
            text,
            "{style:?}"
        );
    }
}

#[test]
fn rejects_malformed_escapes() {
    assert_eq!(
        unescape("\\uZZZZ").unwrap_err().code,
        "encode.invalid_escape"
    );
    assert_eq!(
        unescape("\\ud83d").unwrap_err().code,
        "encode.invalid_escape"
    );
    assert_eq!(
        unescape("\\u{110000}").unwrap_err().code,
        "encode.invalid_escape"
    );
}

#[test]
fn leaves_unrelated_backslashes() {
    assert_eq!(unescape("C:\\path\\n").unwrap(), "C:\\path\\n");
}
