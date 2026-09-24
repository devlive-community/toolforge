use super::*;
use crate::test_support::{jpeg_with_exif, png_with_text};

fn temp(name: &str, data: &[u8]) -> String {
    let dir = std::env::temp_dir().join(format!("tfp-image-info-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, data).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn reports_file_and_exif() {
    let path = temp("photo.jpg", &jpeg_with_exif());
    let report = inspect(Args { path }).unwrap();
    assert_eq!(report.format.as_deref(), Some("JPEG"));
    assert_eq!(
        (report.width, report.height, report.aspect),
        (64, 48, [4, 3])
    );
    assert!(!report.has_alpha && report.can_strip);
    assert!(
        report
            .preview
            .unwrap()
            .starts_with("data:image/png;base64,")
    );
    let summary = report.summary.unwrap();
    assert_eq!(summary.make.as_deref(), Some("ToolForge"));
    assert_eq!(summary.model.as_deref(), Some("Test Cam"));
    assert_eq!(summary.orientation, Some(6));
    let gps = summary.gps.unwrap();
    assert_eq!(
        (gps.latitude, gps.longitude, gps.altitude),
        (-33.86, 151.21, Some(12.5))
    );
    let make = report.exif.iter().find(|e| e.tag == "Make").unwrap();
    assert_eq!(make.value, "ToolForge");
    assert_eq!(report.palette.len(), 2);
}

#[test]
fn png_without_exif_container_still_works() {
    let report = inspect(Args {
        path: temp("text.png", &png_with_text()),
    })
    .unwrap();
    assert_eq!(report.format.as_deref(), Some("PNG"));
    assert!(report.can_strip);
}

#[test]
fn converts_dms() {
    assert_eq!(
        dms(&[40.0, 26.0, 46.0], Some("N")).map(|v| (v * 1e4).round() / 1e4),
        Some(40.4461)
    );
    assert!(dms(&[74.0, 0.0, 21.0], Some("W")).unwrap() < 0.0);
    assert_eq!(dms(&[], None), None);
}

#[test]
fn strips_to_a_new_file() {
    let path = temp("strip-me.jpg", &jpeg_with_exif());
    let report = strip_metadata(StripArgs {
        path: path.clone(),
        output: None,
    })
    .unwrap();
    assert!(report.output.ends_with("strip-me-clean.jpg"));
    assert!(report.orientation_lost);
    assert!(report.saved_bytes > 0);
    let cleaned = inspect(Args {
        path: report.output.clone(),
    })
    .unwrap();
    assert!(cleaned.summary.is_none() && cleaned.exif.is_empty());
    // 第二次输出不会覆盖第一次
    let again = strip_metadata(StripArgs { path, output: None }).unwrap();
    assert!(again.output.ends_with("strip-me-clean (1).jpg"));
}

#[test]
fn reports_errors() {
    assert_eq!(
        inspect(Args {
            path: "/nope.png".into()
        })
        .unwrap_err()
        .code,
        "fs.not_found"
    );
    assert_eq!(
        inspect(Args {
            path: temp("bad.png", b"hello")
        })
        .unwrap_err()
        .code,
        "info.unsupported"
    );
}
