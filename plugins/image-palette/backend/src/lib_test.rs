use image::Rgba;

use super::*;

fn fixture(name: &str) -> String {
    let path = std::env::temp_dir().join(format!(
        "tfp-image-palette-{name}-{}.png",
        std::process::id()
    ));
    RgbaImage::from_fn(2000, 1000, |x, _| {
        if x < 1000 {
            Rgba([255, 0, 0, 255])
        } else {
            Rgba([0, 0, 255, 255])
        }
    })
    .save(&path)
    .unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn opens_picks_and_extracts() {
    let path = fixture("open");
    let tool = ImagePalette::default();
    let opened = tool.call("open", json!({ "path": path })).unwrap();
    assert_eq!(
        (opened["width"].as_u64(), opened["height"].as_u64()),
        (Some(2000), Some(1000))
    );
    assert!(
        opened["preview"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,")
    );
    let picked = tool
        .call("pick", json!({ "path": path, "x": 1500, "y": 10 }))
        .unwrap();
    assert_eq!(picked["hex"], "#0000ff");
    let swatches = tool
        .call("palette", json!({ "path": path, "count": 4 }))
        .unwrap();
    assert_eq!(swatches.as_array().unwrap().len(), 2);
    assert_eq!(
        tool.call("palette", json!({ "path": path, "count": 1 }))
            .unwrap_err()
            .code,
        "palette.invalid_count"
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn exports_and_validates_colors() {
    let tool = ImagePalette::default();
    let out = tool
        .call(
            "export",
            json!({ "colors": ["#FF0000", "00ff00"], "format": "scss", "name": "My Brand!" }),
        )
        .unwrap();
    assert_eq!(
        out["text"],
        "$my-brand-100: #ff0000;\n$my-brand-200: #00ff00;"
    );
    let err = tool
        .call("export", json!({ "colors": ["red"], "format": "css" }))
        .unwrap_err();
    assert_eq!(err.code, "palette.invalid_color");
    assert_eq!(
        tool.call("open", json!({ "path": "/no/such.png" }))
            .unwrap_err()
            .code,
        "fs.not_found"
    );
}
