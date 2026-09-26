use super::*;

#[test]
fn detects_encodings() {
    assert_eq!(detect_encoding(b"plain ascii").unwrap(), UTF_8);
    assert_eq!(detect_encoding("中文日志".as_bytes()).unwrap(), UTF_8);
    // 在多字节字符中间截断
    assert_eq!(detect_encoding(&"中文".as_bytes()[..4]).unwrap(), UTF_8);
    let (gbk, _, _) = GB18030.encode("2026-09-26 ERROR 数据库连接失败");
    assert_eq!(detect_encoding(&gbk).unwrap(), GB18030);
    assert_eq!(
        detect_encoding(b"\xff\xfeE\0").unwrap_err().code,
        "log.utf16_unsupported"
    );
    assert_eq!(detect_encoding(b"bad \xff\xff\xff bytes").unwrap(), UTF_8);
}

#[test]
fn decodes_lines() {
    assert_eq!(decode(b"\xef\xbb\xbfhello", UTF_8), "hello");
    assert_eq!(decode(b"\x1b[31mred\x1b[0m", UTF_8), "red");
    let (gbk, _, _) = GB18030.encode("失败");
    assert_eq!(decode(&gbk, GB18030), "失败");
    assert_eq!(decode(b"a\xffb", UTF_8), "a\u{fffd}b");
}

fn matcher(query: &str, regex: bool, case_sensitive: bool, levels: &[&str]) -> Matcher {
    Matcher::new(&Filter {
        query: query.into(),
        regex,
        case_sensitive,
        levels: levels.iter().map(|l| l.to_string()).collect(),
    })
    .unwrap()
}

#[test]
fn matches_text_and_levels() {
    let plain = matcher("a.b", false, false, &[]);
    assert!(plain.matches("x A.B y") && !plain.matches("axb"));
    assert!(plain.accepts_level(level::NONE));
    let regex = matcher(r"user=\d+", true, true, &["error", "warn"]);
    assert!(regex.matches("user=42") && !regex.matches("USER=42"));
    assert!(regex.accepts_level(level::ERROR) && !regex.accepts_level(level::INFO));
    assert!(matcher("", false, false, &[]).is_empty());
    assert!(!matcher("", false, false, &["info"]).is_empty());
    let bad = Matcher::new(&Filter {
        query: "(".into(),
        regex: true,
        ..Filter::default()
    });
    assert_eq!(bad.err().unwrap().code, "log.invalid_regex");
    let level = Matcher::new(&Filter {
        levels: vec!["loud".into()],
        ..Filter::default()
    });
    assert_eq!(level.err().unwrap().code, "log.invalid_level");
}

#[test]
fn reports_marks_in_utf16_units() {
    let m = matcher("err", false, false, &[]);
    assert_eq!(m.marks("😀 ERR and err"), vec![[3, 6], [11, 14]]);
    assert!(matcher("", false, false, &[]).marks("anything").is_empty());
    assert!(
        matcher("x*", true, false, &[]).marks("abc").is_empty(),
        "empty matches are skipped"
    );
}

#[test]
fn truncates_and_formats_json() {
    assert_eq!(truncate("héllo", 2), ("hé", true));
    assert_eq!(truncate("hi", 5), ("hi", false));
    let pretty = pretty_json(r#"10:00 INFO {"user":"ada","ok":true} done"#).unwrap();
    assert!(pretty.contains("\"user\": \"ada\""));
    assert!(pretty_json("no json {here").is_none());
    assert!(pretty_json("} {").is_none());
}
