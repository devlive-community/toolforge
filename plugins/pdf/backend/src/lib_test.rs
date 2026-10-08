use serde_json::json;

use super::*;
use crate::test_support::{Ctx, encrypted, sample, workspace};

#[test]
fn renders_thumbnails() {
    let dir = workspace("thumbs");
    let path = sample(&dir, "a.pdf", 3);
    let plugin = PdfTools::default();
    let thumbs = plugin
        .call(
            "thumbnails",
            json!({ "path": path, "pages": [1, 3, 9], "width": 120 }),
        )
        .unwrap();
    let thumbs = thumbs.as_array().unwrap();
    assert_eq!(thumbs.len(), 3);
    let uri = thumbs[0]["dataUri"].as_str().unwrap();
    let png = data_encoding::BASE64
        .decode(uri.trim_start_matches("data:image/png;base64,").as_bytes())
        .unwrap();
    let image = image::load_from_memory(&png).unwrap().to_rgb8();
    assert_eq!(image.width(), 120);
    // 第 1 页填充红色
    let center = image.get_pixel(10, 10);
    assert!(
        center[0] > 200 && center[1] < 60 && center[2] < 60,
        "{center:?}"
    );
    // 第 3 页旋转 90 度：缩略图为横向
    let rotated = data_encoding::BASE64
        .decode(
            thumbs[1]["dataUri"]
                .as_str()
                .unwrap()
                .trim_start_matches("data:image/png;base64,")
                .as_bytes(),
        )
        .unwrap();
    let rotated = image::load_from_memory(&rotated).unwrap();
    assert!(
        rotated.width() == 120 && rotated.height() < 120,
        "{}x{}",
        rotated.width(),
        rotated.height()
    );
    assert!(
        thumbs[2]["dataUri"].is_null(),
        "pages out of range have no thumbnail"
    );

    let locked = encrypted(&dir, "e.pdf", "pw");
    assert_eq!(
        plugin
            .call("thumbnails", json!({ "path": locked, "pages": [1] }))
            .unwrap_err()
            .code,
        "pdf.password_required"
    );
    let ok = plugin
        .call(
            "thumbnails",
            json!({ "path": locked, "password": "pw", "pages": [1] }),
        )
        .unwrap();
    assert!(ok[0]["dataUri"].is_string());
}

#[test]
fn runs_compose_split_and_images_tasks() {
    let dir = workspace("tasks");
    let a = sample(&dir, "report.pdf", 5);
    let plugin = PdfTools::default();
    let ctx = Ctx::default();
    let out = dir.join("merged.pdf");
    let result = plugin
        .run_task(
            "compose",
            json!({ "sources": [{ "path": a }], "pages": [{ "source": 0, "page": 5 }, { "source": 0, "page": 1, "rotate": 180 }], "output": out }),
            &ctx,
        )
        .unwrap();
    assert_eq!(result["pages"], 2);
    assert!(out.exists());

    let split = plugin
        .run_task(
            "split",
            json!({ "path": a, "mode": "every", "size": 2 }),
            &ctx,
        )
        .unwrap();
    let outputs: Vec<&str> = split["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o.as_str().unwrap())
        .collect();
    assert_eq!(outputs.len(), 3);
    assert!(
        outputs[0].ends_with("report_p1-2.pdf") && outputs[2].ends_with("report_p5.pdf"),
        "{outputs:?}"
    );
    let ranges = plugin.run_task(
        "split",
        json!({ "path": a, "mode": "ranges", "ranges": "2-3", "outputDir": dir.join("missing") }),
        &ctx,
    );
    assert_eq!(ranges.unwrap_err().code, "pdf.output_dir_missing");

    let missing_dir = plugin.run_task("compose", json!({ "sources": [{ "path": a }], "pages": [{ "source": 0, "page": 1 }], "output": "/no/such/dir/x.pdf" }), &ctx);
    assert_eq!(missing_dir.unwrap_err().code, "pdf.output_dir_missing");
    assert!(ctx.0.lock().unwrap().iter().any(|c| c == "pdf.part_saved"));
}
