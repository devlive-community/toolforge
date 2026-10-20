use super::*;

#[test]
fn exposes_inspect_and_stitch() {
    let plugin = ImageStitcher::default();
    assert_eq!(plugin.manifest().id, "org.devlive.toolforge.image-stitcher");
    assert!(plugin.call("stitch", serde_json::json!({})).is_err());
    assert!(plugin.call("nope", serde_json::json!({})).is_err());
}

#[test]
fn inspects_files_with_errors_reported_per_file() {
    let dir = std::env::temp_dir().join(format!("tfp-stitch-inspect-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let good = dir.join("good.png");
    RgbaImage::from_fn(4, 2, |x, _| image::Rgba([x as u8, 0, 0, 255]))
        .save(&good)
        .unwrap();
    let bad = dir.join("bad.png");
    std::fs::write(&bad, b"not a png").unwrap();

    let plugin = ImageStitcher::default();
    let value = plugin
        .call("inspect", serde_json::json!({ "paths": [good, bad] }))
        .unwrap();
    let entries: Vec<serde_json::Value> = serde_json::from_value(value).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["width"], 4);
    assert_eq!(entries[0]["height"], 2);
    assert!(entries[0]["thumbnail"].is_string());
    assert_eq!(entries[1]["error"]["code"], "stitch.decode_failed");
}
