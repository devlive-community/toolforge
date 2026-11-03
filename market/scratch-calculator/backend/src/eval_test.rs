use super::{Output, evaluate};
use serde_json::json;

fn run(input: &str) -> Output {
    evaluate(input).unwrap()
}

fn value_of(out: &Output, index: usize) -> f64 {
    out.lines[index].value.unwrap()
}

#[test]
fn evaluates_arithmetic_precedence() {
    let out = run("2 + 3 * 4\n(2 + 3) * 4\n2^3^2\n-5 + 10");
    assert_eq!(value_of(&out, 0), 14.0);
    assert_eq!(value_of(&out, 1), 20.0);
    assert_eq!(value_of(&out, 2), 512.0, "power is right associative");
    assert_eq!(value_of(&out, 3), 5.0);
}

#[test]
fn percent_suffix_and_modulo() {
    let out = run("50%\n200 * 10%\n10 % 3");
    assert_eq!(value_of(&out, 0), 0.5);
    assert_eq!(value_of(&out, 1), 20.0);
    assert_eq!(value_of(&out, 2), 1.0);
}

#[test]
fn variables_assign_and_reference() {
    let out = run("x = 6\ny = x * 7\ny + 1");
    assert_eq!(out.lines[0].name.as_deref(), Some("x"));
    assert_eq!(value_of(&out, 1), 42.0);
    assert_eq!(value_of(&out, 2), 43.0);
    assert_eq!(out.last, Some(43.0));
}

#[test]
fn prev_references_last_expression() {
    let out = run("3 * 4\nprev + 1\nanswer = prev * 10");
    assert_eq!(value_of(&out, 1), 13.0);
    // 赋值行不更新 prev，但 prev 仍是 13
    assert_eq!(value_of(&out, 2), 130.0);
}

#[test]
fn units_convert_and_check_dimension() {
    let out = run("1m + 10cm\n1km / 1s\n1kg + 1m");
    assert_eq!(value_of(&out, 0), 1.1);
    assert_eq!(value_of(&out, 1), 1000.0);
    assert_eq!(
        out.lines[2].error.as_deref(),
        Some("eval.dimension_mismatch")
    );
}

#[test]
fn comments_and_blank_lines_are_skipped() {
    let out = run("# 注释\n\n  1 + 1  \n");
    assert_eq!(out.lines.len(), 1);
    assert_eq!(value_of(&out, 0), 2.0);
}

#[test]
fn errors_are_reported_per_line() {
    let out = run("1/0\n2 +\nfoo\n1 + 1");
    assert_eq!(out.lines[0].error.as_deref(), Some("eval.divide_by_zero"));
    assert_eq!(out.lines[1].error.as_deref(), Some("eval.unexpected_end"));
    assert_eq!(out.lines[2].error.as_deref(), Some("eval.unknown_name"));
    assert_eq!(value_of(&out, 3), 2.0, "later lines keep working");
}

#[test]
fn unknown_unit_or_trailing_tokens_fail() {
    let out = run("2 x\n1 2");
    assert_eq!(out.lines[0].error.as_deref(), Some("eval.trailing_input"));
    assert_eq!(out.lines[1].error.as_deref(), Some("eval.trailing_input"));
}

#[test]
fn empty_input_is_rejected() {
    let err = evaluate("  \n# only a comment\n").unwrap_err();
    assert_eq!(err.code, "eval.empty_input");
    let _ = json!(null); // 保持 serde_json 在测试树中被使用
}
