use super::*;

#[test]
fn empty_input_is_reported() {
    assert_eq!(parse("  \n", Mode::Json).unwrap_err().code, "json.empty");
}

#[test]
fn strict_mode_rejects_trailing_comma() {
    let err = parse("[1,2,]", Mode::Json).unwrap_err();
    assert_eq!(err.code, "json.syntax");
    assert_eq!(err.params["line"], 1);
}

#[test]
fn truncated_input_is_eof() {
    assert_eq!(
        parse("{\"a\": [1, 2", Mode::Json).unwrap_err().code,
        "json.eof"
    );
}

#[test]
fn json5_accepts_comments_and_trailing_commas() {
    let value = parse("{ // note\n  a: 'x', b: [1,2,], }", Mode::Json5).unwrap();
    assert_eq!(value["a"], "x");
    assert_eq!(value["b"][1], 2);
}

#[test]
fn json5_error_positions_are_one_based() {
    let err = parse("{\n  a: ,\n}", Mode::Json5).unwrap_err();
    assert_eq!(err.params["line"], 2);
}
