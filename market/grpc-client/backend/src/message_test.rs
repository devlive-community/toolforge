use super::*;
use crate::testkit::pool;

fn item() -> MessageDescriptor {
    pool().get_message_by_name("demo.v1.Item").unwrap()
}

#[test]
fn drafts_follow_protojson() {
    let draft: Value = serde_json::from_str(&draft(&item())).unwrap();
    assert_eq!(draft["id"], "0");
    assert_eq!(draft["big"], "0");
    assert_eq!(draft["name"], "");
    assert_eq!(draft["data"], "");
    assert_eq!(draft["color"], "COLOR_UNSPECIFIED");
    assert_eq!(draft["tags"], serde_json::json!([]));
    assert_eq!(draft["counts"], serde_json::json!({}));
    assert_eq!(draft["at"], "1970-01-01T00:00:00Z");
    assert_eq!(
        draft["price"],
        serde_json::json!({ "currency": "", "units": "0" })
    );
    // oneof 只给第一个分支
    assert!(draft.get("a").is_some() && draft.get("b").is_none());
    // 草稿本身可以直接解析
    parse(&item(), &draft.to_string()).unwrap();
}

#[test]
fn keeps_64_bit_precision_from_strings_and_numbers() {
    for json in [
        r#"{"id": "9007199254740993", "big": "18446744073709551615"}"#,
        r#"{"id": 9007199254740993, "big": 18446744073709551615}"#,
    ] {
        let message = parse(&item(), json).unwrap();
        let back: Value = serde_json::from_str(&to_json(&message).unwrap()).unwrap();
        assert_eq!(back["id"], "9007199254740993");
        assert_eq!(back["big"], "18446744073709551615");
        let decoded = decode_to_json(&item(), &encode(&message)).unwrap();
        assert!(decoded.contains("\"9007199254740993\""));
    }
}

#[test]
fn maps_bytes_enums_and_well_known_types() {
    let message = parse(
        &item(),
        r#"{"data": "AQI=", "color": 1, "at": "2026-10-17T08:00:00Z", "counts": {"x": 2}, "tags": ["a"]}"#,
    )
    .unwrap();
    let back: Value = serde_json::from_str(&to_json(&message).unwrap()).unwrap();
    assert_eq!(back["data"], "AQI=");
    assert_eq!(back["color"], "RED");
    assert_eq!(back["at"], "2026-10-17T08:00:00Z");
    assert_eq!(back["counts"]["x"], 2);
    // 空正文按空消息处理
    assert!(parse(&item(), "  ").is_ok());
}

#[test]
fn rejects_invalid_requests_with_context() {
    for json in [
        r#"{"nope": 1}"#,
        r#"{"color": "PURPLE"}"#,
        r#"{"id": "12x"}"#,
        r#"{"data": "***"}"#,
        r#"{"id": 1} trailing"#,
        "[1]",
    ] {
        let err = parse(&item(), json).unwrap_err();
        assert_eq!(err.code, "grpc.request_invalid", "{json}");
        assert_eq!(err.params["message"], "demo.v1.Item");
    }
    let err = decode_to_json(&item(), &[0xff, 0xff]).unwrap_err();
    assert_eq!(err.code, "grpc.response_invalid");
}
