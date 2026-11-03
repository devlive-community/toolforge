use super::*;

#[test]
fn presets_are_light_to_dark_and_start_light() {
    for (id, chars) in PRESETS {
        assert!(
            chars.starts_with(' '),
            "preset {id} should start with the lightest glyph (space)"
        );
        let unique: std::collections::HashSet<char> = chars.chars().collect();
        assert_eq!(
            unique.len(),
            chars.chars().count(),
            "preset {id} has duplicate glyphs"
        );
        assert!(
            chars.chars().count() >= 2,
            "preset {id} needs at least 2 glyphs"
        );
    }
}

#[test]
fn preset_lookup_is_case_sensitive_and_exact() {
    assert_eq!(preset_chars("classic"), Some(" .:-=+*#%@"));
    assert_eq!(preset_chars("Classic"), None);
    assert_eq!(preset_chars("missing"), None);
}

#[test]
fn custom_chars_keep_order_and_merge_duplicates() {
    let (normalized, stats) = normalize_custom("aab b@").expect("valid custom glyphs");
    // 输入含空格（可打印、合法）：a、b、空格、@ 按首现顺序保留
    assert_eq!(normalized, "ab @");
    assert_eq!(stats.duplicates, 2);
    assert_eq!(stats.skipped, 0);
}

#[test]
fn custom_chars_skip_controls_and_non_ascii() {
    let (normalized, stats) = normalize_custom(" .\n\t-中é").expect("valid custom glyphs");
    assert_eq!(normalized, " .-");
    // \n 与 \t 是控制字符，中、é 是非 ASCII，共跳过 4 个
    assert_eq!(stats.skipped, 4);
}

#[test]
fn custom_chars_require_two_distinct_printable() {
    let error = normalize_custom("aa").expect_err("duplicates collapse to one char");
    assert_eq!(error.code, "ascii.bad_custom");
    let error = normalize_custom("中").expect_err("non-ascii only");
    assert_eq!(error.code, "ascii.bad_custom");
}
