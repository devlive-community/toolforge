use super::*;
use serde_json::json;

#[test]
fn transform_is_callable_and_file_encoding_is_a_task() {
    let tool = Encoder::default();
    let out = tool
        .call(
            "transform",
            json!({"input": "foobar", "codec": "base64", "direction": "encode"}),
        )
        .unwrap();
    assert_eq!(out["output"], "Zm9vYmFy");
    assert!(tool.manifest().functions["encode_file"].task);
    assert_eq!(
        tool.call("file_output", json!({"id": 1})).unwrap_err().code,
        "encode.result_expired"
    );
}
