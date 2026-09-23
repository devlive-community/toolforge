use super::*;

fn test_with(pattern: &str, flags: Flags, input: &str) -> PluginResult<TestOutput> {
    test(TestArgs {
        pattern: pattern.into(),
        flags,
        input: input.into(),
    })
}

fn run(pattern: &str, input: &str) -> TestOutput {
    test_with(pattern, Flags::default(), input).unwrap()
}

#[test]
fn finds_matches_with_numbered_and_named_groups() {
    let out = run(r"(?P<user>\w+)@(\w+)\.com", "a@x.com, bob@yy.com");
    assert_eq!(out.count, 2);
    assert_eq!(out.group_names, vec![None, Some("user".into()), None]);
    let second = &out.matches[1];
    assert_eq!(
        (second.start, second.end, second.text.as_str()),
        (9, 19, "bob@yy.com")
    );
    let user = second.groups[0].as_ref().unwrap();
    assert_eq!(
        (user.name.as_deref(), user.text.as_str()),
        (Some("user"), "bob")
    );
    assert_eq!(second.groups[1].as_ref().unwrap().text, "yy");
}

#[test]
fn optional_groups_that_do_not_participate_are_none() {
    let out = run(r"a(b)?c", "ac");
    assert!(out.matches[0].groups[0].is_none());
}

#[test]
fn offsets_are_utf16_for_cjk_and_emoji() {
    let input = "😀 你好 world";
    let out = run("你好", input);
    let m = &out.matches[0];
    let expected_start = "😀 ".encode_utf16().count();
    assert_eq!((m.start, m.end), (expected_start, expected_start + 2));
}

#[test]
fn flags_change_matching() {
    let flags = Flags {
        case_insensitive: true,
        multi_line: true,
        ..Flags::default()
    };
    let out = test_with(r"^hello$", flags, "x\nHELLO\ny").unwrap();
    assert_eq!(out.count, 1);
    let dot_all = Flags {
        dot_all: true,
        ..Flags::default()
    };
    assert_eq!(test_with("a.b", dot_all, "a\nb").unwrap().count, 1);
    assert_eq!(run("a.b", "a\nb").count, 0);
    let extended = Flags {
        extended: true,
        ..Flags::default()
    };
    assert_eq!(
        test_with(r"a b  # comment", extended, "ab").unwrap().count,
        1
    );
}

#[test]
fn supports_lookaround_and_backreferences() {
    assert_eq!(run(r"\d+(?=px)", "10px 20em 30px").count, 2);
    assert_eq!(run(r"(?<!\$)\b\d+", "$5 7").matches[0].text, "7");
    assert_eq!(run(r"(\w)\1", "aa bc dd").count, 2);
}

#[test]
fn invalid_and_empty_patterns() {
    let err = test_with("(abc", Flags::default(), "abc").unwrap_err();
    assert_eq!(err.code, "regex.invalid");
    assert!(err.params.contains_key("detail"));
    assert_eq!(
        test_with("", Flags::default(), "abc").unwrap_err().code,
        "regex.empty"
    );
}

#[test]
fn catastrophic_backtracking_is_stopped() {
    let input = format!("{}!", "a".repeat(40));
    let err = test_with(r"(a*)*\1b", Flags::default(), &input).unwrap_err();
    assert_eq!(err.code, "regex.backtrack_limit");
}

#[test]
fn match_list_is_capped_but_count_is_exact() {
    let input = "x".repeat(MAX_MATCHES + 5);
    let out = run("x", &input);
    assert_eq!(out.count, MAX_MATCHES + 5);
    assert_eq!(out.matches.len(), MAX_MATCHES);
    assert!(out.truncated);
}

#[test]
fn replaces_with_numbered_and_named_references() {
    let out = replace(ReplaceArgs {
        pattern: r"(?P<y>\d{4})-(\d{2})".into(),
        flags: Flags::default(),
        input: "2024-02 and 2025-11".into(),
        replacement: "$2/${y}".into(),
    })
    .unwrap();
    assert_eq!(out.output, "02/2024 and 11/2025");
    assert_eq!(out.count, 2);
}
