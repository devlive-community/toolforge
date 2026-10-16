use super::*;

fn count(repaired: &Repaired, code: &str) -> usize {
    repaired
        .repairs
        .iter()
        .find(|repair| repair.code == code)
        .map_or(0, |repair| repair.count)
}

#[test]
fn repairs_issue_353_python_and_extra_object_commas() {
    let repaired =
        repair("{name: 'ToolForge',, active: True, missing: None, enabled: False,}").unwrap();
    assert_eq!(
        repaired.value,
        serde_json::json!({"name": "ToolForge", "active": true, "missing": null, "enabled": false})
    );
    assert_eq!(count(&repaired, "json.repair.unquoted_keys"), 4);
    assert_eq!(count(&repaired, "json.repair.single_quotes"), 1);
    assert_eq!(count(&repaired, "json.repair.extra_commas"), 2);
    assert_eq!(count(&repaired, "json.repair.python_literals"), 3);
}

#[test]
fn valid_json_has_no_repairs_and_keeps_member_order() {
    let input = r#"{"z": 1, "a": [true, null, "None"]}"#;
    let repaired = repair(input).unwrap();
    assert!(repaired.repairs.is_empty());
    assert_eq!(
        repaired.value,
        serde_json::from_str::<Value>(input).unwrap()
    );
    assert_eq!(
        repaired.value.as_object().unwrap().keys().next().unwrap(),
        "z"
    );
}

#[test]
fn strings_do_not_interpret_literals_comments_or_punctuation() {
    let input = r#"{"text":"None True False undefined //comment /*note*/ , , 'single'", "url":"https://a.test/x"}"#;
    let repaired = repair(input).unwrap();
    assert!(repaired.repairs.is_empty());
    assert_eq!(
        repaired.value,
        serde_json::from_str::<Value>(input).unwrap()
    );
}

#[test]
fn escaped_single_quotes_double_quotes_and_unicode_are_preserved() {
    let repaired =
        repair(r#"{'say': 'don\'t say "None"', 名字: '测试 🦀', 'emoji': '\uD83E\uDD80'}"#)
            .unwrap();
    assert_eq!(repaired.value["say"], "don't say \"None\"");
    assert_eq!(repaired.value["名字"], "测试 🦀");
    assert_eq!(repaired.value["emoji"], "🦀");
}

#[test]
fn json5_style_string_escapes_and_line_continuations_are_explicit() {
    let repaired = repair("['\\x41\\v\\0a\\\ncontinued', '\\\r\nnext']").unwrap();
    assert_eq!(repaired.value[0], "A\u{b}\0acontinued");
    assert_eq!(repaired.value[1], "next");
    assert_eq!(count(&repaired, "json.repair.string_escapes"), 5);
}

#[test]
fn raw_controls_inside_strings_are_escaped_without_deleting_characters() {
    let repaired = repair("[\"a\nb\tc\u{0}d\"]").unwrap();
    assert_eq!(repaired.value[0], "a\nb\tc\0d");
    assert_eq!(count(&repaired, "json.repair.control_characters"), 3);
    let output = serde_json::to_string(&repaired.value).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&output).unwrap(),
        repaired.value
    );
}

#[test]
fn invalid_or_unclosed_strings_are_not_guessed() {
    for input in [
        "{'name': 'unfinished}",
        r#"{"x": "\q"}"#,
        r#"['\01']"#,
        r#"['\uD800']"#,
        r#"['\uDC00']"#,
        r#"['\u12xz']"#,
        r#"['\xGG']"#,
        "['don't']",
    ] {
        assert_eq!(
            repair(input).unwrap_err().code,
            "json.repair_syntax",
            "{input}"
        );
    }
}

#[test]
fn removes_comments_only_outside_strings() {
    let repaired = repair("/*before*/ { /* key */ a: 1, // after\n b: 2 } //tail").unwrap();
    assert_eq!(repaired.value, serde_json::json!({"a": 1, "b": 2}));
    assert_eq!(count(&repaired, "json.repair.comments"), 4);
    assert_eq!(
        repair("{a:1/*unfinished").unwrap_err().code,
        "json.repair_syntax"
    );
}

#[test]
fn line_comments_end_at_cr_lf_and_unicode_line_terminators() {
    for newline in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}"] {
        let input = format!("{{a:1,// note{newline}b:2}}");
        let repaired = repair(&input).unwrap();
        assert_eq!(repaired.value, serde_json::json!({"a": 1, "b": 2}));
        assert_eq!(count(&repaired, "json.repair.comments"), 1);
    }
}

#[test]
fn removes_object_extra_commas_with_exact_count() {
    for (input, expected) in [
        ("{,,,a:1,,,b:2,,,}", 8),
        ("{,,,}", 3),
        ("{a:1,,,}", 3),
        ("{a:1,b:2}", 0),
    ] {
        let repaired = repair(input).unwrap();
        assert_eq!(
            count(&repaired, "json.repair.extra_commas"),
            expected,
            "{input}"
        );
    }
}

#[test]
fn arrays_allow_one_trailing_comma_without_dropping_holes() {
    let repaired = repair("[1,2,]").unwrap();
    assert_eq!(repaired.value, serde_json::json!([1, 2]));
    assert_eq!(count(&repaired, "json.repair.extra_commas"), 1);
    for input in ["[,1]", "[,]", "[1,,2]", "[1,,]", "[1,", "[1,,", "[,]"] {
        assert_eq!(
            repair(input).unwrap_err().code,
            "json.repair_syntax",
            "{input}"
        );
    }
}

#[test]
fn missing_commas_are_inserted_at_clear_value_boundaries() {
    let repaired = repair(r#"{a: [1 2 "x"{}[]true false null] b: None}"#).unwrap();
    assert_eq!(
        repaired.value,
        serde_json::json!({"a": [1, 2, "x", {}, [], true, false, null], "b": null})
    );
    assert_eq!(count(&repaired, "json.repair.missing_commas"), 8);
}

#[test]
fn merged_tokens_arithmetic_unknown_values_and_multi_root_are_rejected() {
    for input in [
        "[1true]",
        "[truefalse]",
        "[1-2]",
        "[01]",
        "[1.2.3]",
        "[NoneTrue]",
        "[unknown]",
        "[NaN]",
        "[Infinity]",
        "{a:foo}",
        "{}{}",
        "1 2",
        "[+1]",
        "[0x10]",
        "{'x': __import__('os').system('echo test')}",
    ] {
        assert_eq!(
            repair(input).unwrap_err().code,
            "json.repair_syntax",
            "{input}"
        );
    }
}

#[test]
fn closes_only_containers_with_complete_values() {
    let repaired = repair("{a: [1, {b: true}").unwrap();
    assert_eq!(repaired.value, serde_json::json!({"a": [1, {"b": true}]}));
    assert_eq!(count(&repaired, "json.repair.unclosed_containers"), 2);
    for input in ["{", "[", "[{", "{a:", "{a:1,", "{a:1 b:", "[1,}"] {
        assert_eq!(
            repair(input).unwrap_err().code,
            "json.repair_syntax",
            "{input}"
        );
    }
}

#[test]
fn missing_colons_and_mismatched_closers_are_rejected() {
    for input in ["{a 1}", "{a:}", "{a:1]", "[1}", "{a:[1}", "{a:1, :2}"] {
        assert_eq!(
            repair(input).unwrap_err().code,
            "json.repair_syntax",
            "{input}"
        );
    }
}

#[test]
fn duplicate_keys_are_rejected_after_decoding_instead_of_overwritten() {
    for input in [
        "{a:1, a:2}",
        r#"{"name":1, '\u006eame':2}"#,
        r#"{"x":1, x:2}"#,
        r#"{"🦀":1, '\uD83E\uDD80':2}"#,
    ] {
        let error = repair(input).unwrap_err();
        assert_eq!(error.code, "json.repair_duplicate_key");
        assert!(error.params["key"].is_string());
        assert!(error.params["line"].is_number());
        assert!(error.params["column"].is_number());
    }
}

#[test]
fn integers_and_decimal_exponents_keep_their_source_precision() {
    let numbers = [
        "9007199254740993",
        "184467440737095516160000000000000000001",
        "-123456789012345678901234567890",
        "0.123456789012345678901234567890",
        "1.234567890123456789000e+123",
        "1e+9999999999999999999999999999",
    ];
    for number in numbers {
        let repaired = repair(&format!("{{value:{number},}}")).unwrap();
        assert_eq!(repaired.value["value"].to_string(), number);
    }
}

#[test]
fn undefined_is_explicitly_counted_as_null_and_properties_are_kept() {
    let repaired = repair("{a:undefined, b:[undefined, None, True, False]}").unwrap();
    assert_eq!(
        repaired.value,
        serde_json::json!({"a":null, "b":[null, null, true, false]})
    );
    assert_eq!(count(&repaired, "json.repair.undefined"), 2);
    assert_eq!(count(&repaired, "json.repair.python_literals"), 3);
}

#[test]
fn bom_is_removed_only_at_start() {
    let repaired = repair("\u{feff}{a:1}").unwrap();
    assert_eq!(count(&repaired, "json.repair.bom"), 1);
    assert!(repair("{a:\u{feff}1}").is_err());
}

#[test]
fn error_locations_use_original_text_and_utf16_columns() {
    for newline in ["\r\n", "\r", "\n"] {
        let error = repair(&format!("{{{newline}  '中文🦀' 1}}")).unwrap_err();
        assert_eq!(error.params["line"], 2);
        assert_eq!(error.params["column"], 10);
        assert_eq!(error.params["reason"], "expected_colon");
    }
}

#[test]
fn size_node_and_depth_limits_are_enforced_before_unbounded_work() {
    assert_eq!(
        repair(&" ".repeat(MAX_BYTES + 1)).unwrap_err().code,
        "json.repair_too_large"
    );
    let too_many = format!("[{}]", vec!["0"; MAX_NODES].join(","));
    assert_eq!(
        repair(&too_many).unwrap_err().code,
        "json.repair_too_many_nodes"
    );
    let allowed = format!("{}0{}", "[".repeat(MAX_DEPTH), "]".repeat(MAX_DEPTH));
    let repaired = repair(&allowed).unwrap();
    assert!(
        crate::parse::parse(
            &serde_json::to_string(&repaired.value).unwrap(),
            crate::Mode::Json,
        )
        .is_ok()
    );
    let too_deep = format!(
        "{}0{}",
        "[".repeat(MAX_DEPTH + 1),
        "]".repeat(MAX_DEPTH + 1)
    );
    assert_eq!(repair(&too_deep).unwrap_err().code, "json.repair_too_deep");
}

#[test]
fn empty_inputs_and_non_json_whitespace_are_not_silently_repaired() {
    assert_eq!(repair(" \n\t\r").unwrap_err().code, "json.empty");
    for input in ["[1\u{b}2]", "[1\u{c}2]", "\u{a0}{}"] {
        assert_eq!(repair(input).unwrap_err().code, "json.repair_syntax");
    }
}
