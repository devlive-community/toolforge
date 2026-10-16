use super::*;

/// 测试输入中用 `U+` 代替反斜杠 u 转义，运行时再替换
fn esc(input: &str) -> String {
    input.replace("U+", "\\u")
}

fn canon(input: &str) -> String {
    canonicalize(input).unwrap().text
}

#[test]
fn matches_rfc_8785_example() {
    // RFC 8785 §3.2.2 示例
    let input = esc(r#"{
  "numbers": [333333333.33333329, 1E30, 4.50, 2e-3, 0.000000000000000000000000001],
  "string": "U+20ac$U+000FU+000aA'U+0042U+0022U+005c\\\"\/",
  "literals": [null, true, false]
}"#);
    assert_eq!(
        canon(&input),
        r#"{"literals":[null,true,false],"numbers":[333333333.3333333,1e+30,4.5,0.002,1e-27],"string":"€$\u000f\nA'B\"\\\\\"/"}"#
    );
}

#[test]
fn sorts_keys_by_utf16_code_units() {
    // RFC 8785 §3.2.3：😀（D83D DE00）排在 U+FB33 之前，与按码位排序不同
    let input = esc(r#"{
  "U+20ac": "Euro Sign",
  "\r": "Carriage Return",
  "U+fb33": "Hebrew Letter Dalet With Dagesh",
  "1": "One",
  "U+d83dU+de00": "Emoji: Grinning Face",
  "U+0080": "Control",
  "U+00f6": "Latin Small Letter O With Diaeresis"
}"#);
    let out = canon(&input);
    let keys = ["\\r", "1", "\u{80}", "ö", "€", "😀", "\u{fb33}"];
    let positions: Vec<usize> = keys
        .iter()
        .map(|k| out.find(&format!("\"{k}\":")).unwrap())
        .collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]), "{out}");
}

#[test]
fn serializes_numbers_like_ecmascript() {
    let cases: [(u64, &str); 12] = [
        (0x0000000000000000, "0"),
        (0x8000000000000000, "0"),
        (0x0000000000000001, "5e-324"),
        (0x8000000000000001, "-5e-324"),
        (0x7fefffffffffffff, "1.7976931348623157e+308"),
        (0xffefffffffffffff, "-1.7976931348623157e+308"),
        (0x4340000000000000, "9007199254740992"),
        (0xc340000000000000, "-9007199254740992"),
        (0x4430000000000000, "295147905179352830000"),
        (0x44b52d02c7e14af6, "1e+23"),
        (0x444b1ae4d6e2ef50, "1e+21"),
        (0x3eb0c6f7a0b5ed8d, "0.000001"),
    ];
    for (bits, expected) in cases {
        assert_eq!(format_number(f64::from_bits(bits)), expected, "{bits:#x}");
    }
    assert_eq!(format_number(1e-7), "1e-7");
    assert_eq!(format_number(123.456), "123.456");
    assert_eq!(format_number(1e20), "100000000000000000000");
}

#[test]
fn reports_precision_changes() {
    let out = canonicalize("[1.0, 1.50, 2e3, 12345678901234567890, 0.1]").unwrap();
    assert_eq!(out.text, "[1,1.5,2000,12345678901234567000,0.1]");
    assert_eq!(out.lossy_numbers, 1);
}

#[test]
fn rejects_duplicate_keys_lone_surrogates_and_overflow() {
    let err = canonicalize("{\"a\":1,\n \"a\":2}").unwrap_err();
    assert_eq!(err.code, "json.duplicate_key");
    assert_eq!(err.params["key"], "a");
    assert_eq!(err.params["line"], 2);
    // 转义后相同的键也算重复
    assert_eq!(
        canonicalize(&esc(r#"{"a":1,"U+0061":2}"#))
            .unwrap_err()
            .code,
        "json.duplicate_key"
    );
    for input in [r#""U+d800""#, r#""U+dc00x""#, r#""U+d800U+0041""#].map(esc) {
        assert_eq!(
            canonicalize(&input).unwrap_err().code,
            "json.lone_surrogate"
        );
    }
    assert_eq!(canonicalize("1e400").unwrap_err().code, "json.number_range");
}

#[test]
fn rejects_non_strict_json() {
    for input in [
        "{a:1}", "[1,]", "01", "1.", "'x'", "[1] x", "\"a\tb\"", "NaN",
    ] {
        assert!(canonicalize(input).is_err(), "{input}");
    }
    assert_eq!(canonicalize("[").unwrap_err().code, "json.eof");
    assert_eq!(canonicalize("  ").unwrap_err().code, "json.empty");
    let deep = "[".repeat(600) + &"]".repeat(600);
    assert_eq!(canonicalize(&deep).unwrap_err().code, "json.too_deep");
}

#[test]
fn keeps_array_order_and_unicode_as_is() {
    // 不做 Unicode 归一化：é 的组合形式与预组合形式保持不同
    assert_eq!(canon("[3,1,2]"), "[3,1,2]");
    assert_eq!(
        canon("[\"e\u{301}\",\"\u{e9}\"]"),
        "[\"e\u{301}\",\"\u{e9}\"]"
    );
    assert_eq!(
        canon(r#"{"b":{"d":1,"c":2},"a":[]}"#),
        r#"{"a":[],"b":{"c":2,"d":1}}"#
    );
}
