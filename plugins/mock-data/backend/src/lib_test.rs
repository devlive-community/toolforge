use serde_json::json;

use super::*;

fn args() -> Value {
    json!({
        "locale": "zh-cn",
        "rows": 500,
        "seed": 7,
        "fields": [
            { "name": "id", "kind": "id" },
            { "name": "name", "kind": "name" },
            { "name": "phone", "kind": "phone" }
        ],
        "format": "csv"
    })
}

#[test]
fn previews_and_saves_the_same_data() {
    let tool = MockData::default();
    let preview = tool.call("generate", args()).unwrap();
    assert_eq!(
        (preview["rows"].as_u64(), preview["previewRows"].as_u64()),
        (Some(500), Some(200))
    );
    assert_eq!(preview["seed"], 7);
    let preview_text = preview["output"].as_str().unwrap().to_owned();
    assert_eq!(preview_text.lines().count(), 201);

    let path = std::env::temp_dir().join(format!("tfp-mock-{}.csv", std::process::id()));
    let mut save = args();
    save["path"] = json!(path.to_string_lossy());
    let saved = tool.call("save", save).unwrap();
    assert_eq!(saved["rows"], 500);
    let full = std::fs::read_to_string(&path).unwrap();
    assert_eq!(full.lines().count(), 501);
    assert!(
        full.starts_with(&preview_text),
        "preview must be the start of the saved file"
    );
}

#[test]
fn reports_errors() {
    let tool = MockData::default();
    let mut sql = args();
    sql["format"] = json!("sql");
    sql["table"] = json!(" ");
    assert_eq!(
        tool.call("generate", sql).unwrap_err().code,
        "mock.no_table"
    );
    let mut unseeded = args();
    unseeded.as_object_mut().unwrap().remove("seed");
    unseeded["path"] = json!("/tmp/x.csv");
    assert_eq!(
        tool.call("save", unseeded).unwrap_err().code,
        "mock.no_seed"
    );
    // 没有种子时随机生成并返回
    let mut random = args();
    random.as_object_mut().unwrap().remove("seed");
    assert!(tool.call("generate", random).unwrap()["seed"].is_u64());
}
