use serde_json::json;

use super::*;

fn convert_value(input: &str) -> Value {
    NumberWords::default()
        .call("convert", json!({ "input": input }))
        .unwrap()
}

fn find<'a>(out: &'a Value, kind: &str) -> &'a Value {
    out["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["kind"] == kind)
        .unwrap()
}

#[test]
fn recognises_each_input_kind() {
    assert_eq!(convert_value("1234.56")["source"], "arabic");
    assert_eq!(convert_value("一千二百")["source"], "chinese");
    assert_eq!(convert_value("壹佰元整")["source"], "rmb");
    assert_eq!(convert_value("MMXXVI")["source"], "roman");
    assert_eq!(convert_value("MMXXVI")["value"], "2026");
    let err = |input: &str| {
        NumberWords::default()
            .call("convert", json!({ "input": input }))
            .unwrap_err()
            .code
    };
    assert_eq!(err("  "), "num.empty");
    assert_eq!(err("hello"), "num.unrecognized");
}

#[test]
fn produces_every_output_with_notes() {
    let out = convert_value("1234.567");
    assert_eq!(find(&out, "rmb")["value"], "壹仟贰佰叁拾肆元伍角柒分");
    assert_eq!(find(&out, "rmb")["note"]["code"], "num.rounded_cents");
    assert_eq!(
        find(&out, "chineseLower")["value"],
        "一千二百三十四点五六七"
    );
    assert_eq!(find(&out, "thousands")["value"], "1,234.567");
    assert_eq!(find(&out, "roman")["value"], Value::Null);
    assert_eq!(find(&out, "roman")["note"]["code"], "num.roman_range");
    assert_eq!(find(&out, "bytes")["note"]["code"], "num.bytes_integer");
    let bytes = convert_value("1572864");
    assert_eq!(find(&bytes, "bytes")["value"], "1.5 MiB · 1.57 MB");
    assert_eq!(find(&bytes, "roman")["note"]["code"], "num.roman_range");
    assert_eq!(find(&convert_value("12"), "roman")["value"], "XII");
}
