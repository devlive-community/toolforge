use super::*;
use serde_json::json;

#[test]
fn generates_and_saves() {
    let tool = BarcodeTool::default();
    let out = tool
        .call(
            "generate",
            json!({ "format": "ean13", "text": "690123456789" }),
        )
        .unwrap();
    assert_eq!(out["content"], "6901234567892");
    assert_eq!(out["checkDigit"], "2");
    assert!(out["svg"].as_str().unwrap().starts_with("<svg"));
    let dir = std::env::temp_dir().join(format!("tf-barcode-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for ext in ["png", "svg"] {
        let path = dir.join(format!("code.{ext}"));
        tool.call(
            "save",
            json!({ "format": "code128", "text": "abc", "saveAs": ext, "path": path }),
        )
        .unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() > 100);
    }
    std::fs::remove_dir_all(dir).ok();
    let err = tool
        .call("generate", json!({ "format": "ean8", "text": "1234" }))
        .unwrap_err();
    assert_eq!(err.code, "barcode.length");
}

#[test]
fn detects_retail_numbers_with_valid_check_digits() {
    let tool = BarcodeTool::default();
    let label = |text: &str| {
        tool.detect(text)
            .map(|d| serde_json::to_value(d).unwrap()["label"].clone())
    };
    assert_eq!(label(" 6901234567892 "), Some(json!("ean13")));
    assert_eq!(label("96385074"), Some(json!("ean8")));
    assert_eq!(label("036000291452"), Some(json!("upcA")));
    assert_eq!(label("6901234567891"), None);
    assert_eq!(label("69012345678a2"), None);
    assert!(tool.detect_image(300, 120).is_some());
    assert!(tool.detect_image(300, 10).is_none());
}
