use super::*;

#[test]
fn parses_code_point_notations() {
    for query in [
        "U+1F600",
        "u+1f600",
        "0x1F600",
        "&#x1F600;",
        "&#128512;",
        "\\u{1F600}",
        "\\U0001F600",
        "😀",
    ] {
        let found = lookup(query);
        assert_eq!(found.len(), 1, "{query}");
        assert_eq!(found[0].code, "U+1F600", "{query}");
    }
    assert!(lookup("U+110000").is_empty() || lookup("U+110000")[0].code != "U+110000");
    assert!(lookup("   ").is_empty());
}

#[test]
fn searches_by_name() {
    let exact = lookup("snowman");
    assert_eq!(exact[0].char, "☃", "the exact name comes first");
    let cats = lookup("cat face");
    assert_eq!(cats[0].char, "🐱");
    assert!(
        cats.iter().any(|f| f.char == "😸") && cats.len() > 5,
        "{}",
        cats.len()
    );
    let words = lookup("greek small alpha");
    assert!(words.iter().any(|f| f.char == "α"));
    assert!(words.len() <= MAX_RESULTS);
    let several = lookup("中文");
    assert_eq!(several.len(), 2);
}

#[test]
fn builds_escapes() {
    let emoji = escapes('😀');
    assert_eq!(emoji.rust, "\\u{1F600}");
    assert_eq!(emoji.javascript, "\\u{1F600}");
    assert_eq!(emoji.python, "\\U0001F600");
    assert_eq!(emoji.java, "\\uD83D\\uDE00");
    assert_eq!(emoji.html, "&#x1F600;");
    assert_eq!(emoji.css, "\\1F600");
    assert_eq!(emoji.url, "%F0%9F%98%80");
    let han = escapes('中');
    assert_eq!(
        (
            han.javascript.as_str(),
            han.python.as_str(),
            han.url.as_str()
        ),
        ("\\u4E2D", "\\u4E2D", "%E4%B8%AD")
    );
}
