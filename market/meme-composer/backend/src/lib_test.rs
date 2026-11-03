use super::*;
use serde_json::json;

fn write_png(path: &std::path::Path) -> std::path::PathBuf {
    let image = image::RgbaImage::from_pixel(100, 80, image::Rgba([90, 90, 200, 255]));
    image.save(path).unwrap();
    path.to_path_buf()
}

fn compose_sample(top: &str, bottom: &str) -> PluginResult<Output> {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "tf-meme-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = write_png(&root.join("photo.png"));
    let out = compose(Args {
        image_path: path.to_string_lossy().into_owned(),
        top_text: Some(top.into()),
        bottom_text: Some(bottom.into()),
        width: None,
        text_color: None,
        stroke_color: None,
    });
    std::fs::remove_dir_all(root).ok();
    out
}

#[test]
fn composes_with_wrapped_lines() {
    let out = compose_sample("上班前", "下班后").unwrap();
    assert_eq!(out.wrapped, 2);
    assert!(out.svg.contains("<image"), "image embedded");
    assert!(out.svg.contains("上班前"));
    assert!(
        out.svg.contains("paint-order='stroke'"),
        "stroke behind fill"
    );
    assert_eq!(out.width, 600);
}

#[test]
fn long_text_wraps_into_lines() {
    let long = "很长的一句吐槽".repeat(20);
    let out = compose_sample(&long, "").unwrap();
    assert!(out.wrapped > 2, "long text wraps");
}

#[test]
fn overflow_is_rejected_not_truncated() {
    // 超过 200 字符上限属于文本过长；换行后仍放不下属于溢出
    let long = "超出画布的很长文字".repeat(60);
    let err = compose_sample("", &long).unwrap_err();
    assert_eq!(err.code, "meme.text_too_long", "200-char cap fires first");
    // 溢出：在 200 字以内但行数超画布（宽 600 → 每行 ~46 全角字，10 行左右即超 160px 文本区）
    let lines = "句".repeat(90);
    let err = compose_sample("", &lines).unwrap_err();
    assert_eq!(err.code, "meme.text_overflow");
}

#[test]
fn validates_inputs() {
    let root = std::env::temp_dir().join(format!("tf-meme-bad-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("bad.png");
    std::fs::write(&path, b"not an image").unwrap();
    let err = compose(Args {
        image_path: path.to_string_lossy().into_owned(),
        top_text: None,
        bottom_text: None,
        width: None,
        text_color: None,
        stroke_color: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "meme.bad_image");
    std::fs::remove_dir_all(root).ok();
    let err = compose(Args {
        image_path: "/nonexistent/x.png".into(),
        top_text: None,
        bottom_text: None,
        width: None,
        text_color: None,
        stroke_color: None,
    })
    .unwrap_err();
    assert!(matches!(
        err.code.as_str(),
        "meme.read_failed" | "fs.not_found"
    ));
}

#[test]
fn base64_matches_known_vectors() {
    assert_eq!(base64_encode(b""), "");
    assert_eq!(base64_encode(b"f"), "Zg==");
    assert_eq!(base64_encode(b"fo"), "Zm8=");
    assert_eq!(base64_encode(b"foo"), "Zm9v");
    assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
    assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
    assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
}

#[test]
fn dispatches_via_plugin() {
    assert_eq!(
        MemeComposer::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&MemeComposer::default()).unwrap();
    assert_eq!(checked, 1);
}
