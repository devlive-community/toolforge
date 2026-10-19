use super::*;
use crate::descriptor::build as b;

/// 插件级链路：内联 base64 输入（无 TaskContext）
#[test]
fn diff_via_call_with_inline_input() {
    let plugin = ProtobufDiff::default();
    let old = b::file_set(&[b::file(
        "demo.proto",
        "demo",
        &[b::message(
            "User",
            &[
                b::field("id", 1, 1, 5, None),
                b::field("name", 2, 1, 9, None),
            ],
            &[],
            &[],
        )],
        &[],
        &[],
    )]);
    let new = b::file_set(&[b::file(
        "demo.proto",
        "demo",
        &[b::message(
            "User",
            &[
                b::field("id", 1, 1, 9, None),
                b::field("name", 2, 1, 9, None),
            ],
            &[],
            &[],
        )],
        &[],
        &[],
    )]);
    let encode = |data: &[u8]| {
        let mut out = String::new();
        // 简易 base64：逐 3 字节组
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        for chunk in data.chunks(3) {
            let b0 = chunk[0] as u32;
            let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
            let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
            let n = (b0 << 16) | (b1 << 8) | b2;
            out.push(TABLE[(n >> 18) as usize & 63] as char);
            out.push(TABLE[(n >> 12) as usize & 63] as char);
            out.push(if chunk.len() > 1 {
                TABLE[(n >> 6) as usize & 63] as char
            } else {
                '='
            });
            out.push(if chunk.len() > 2 {
                TABLE[n as usize & 63] as char
            } else {
                '='
            });
        }
        out
    };
    let args = serde_json::json!({
        "oldText": encode(&old),
        "newText": encode(&new),
    });
    let report = plugin.call("diff", args).unwrap();
    assert_eq!(report["rulesVersion"], "1");
    let findings = report["findings"].as_array().unwrap();
    assert!(
        findings
            .iter()
            .any(|f| f["rule"] == "field.wire_type_changed")
    );
    assert_eq!(findings[0]["severity"], "breaking");

    // export 函数
    let exported = plugin
        .call(
            "export",
            serde_json::json!({ "format": "text", "report": report }),
        )
        .unwrap();
    assert_eq!(exported["ext"], "txt");
    assert!(exported["content"].as_str().unwrap().contains("breaking"));

    // 缺输入
    let err = plugin.call("diff", serde_json::json!({})).unwrap_err();
    assert_eq!(err.code, "pbd.missing_input");
    // 未知函数
    let err = plugin.call("nope", Value::Null).unwrap_err();
    assert_eq!(err.code, "plugin.function_not_found");
}
