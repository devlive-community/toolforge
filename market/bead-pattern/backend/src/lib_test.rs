use super::*;
use serde_json::json;

fn write_png(
    path: &std::path::Path,
    width: u32,
    height: u32,
    pixel: [u8; 4],
) -> std::path::PathBuf {
    let image = image::RgbaImage::from_pixel(width, height, image::Rgba(pixel));
    image.save(path).unwrap();
    path.to_path_buf()
}

#[test]
fn maps_solid_image_to_single_color() {
    let root = std::env::temp_dir().join(format!("tf-bead-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = write_png(&root.join("solid.png"), 20, 20, [255, 0, 0, 255]);
    let out = generate(Args {
        path: path.to_string_lossy().into_owned(),
        columns: 8,
        rows: 8,
        max_colors: Some(6),
        keep_transparent: false,
    })
    .unwrap();
    assert_eq!(out.total_beads, 64);
    assert!(
        out.usage.iter().all(|u| u.color == "red"),
        "pure red maps to red bead"
    );
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn usage_sums_equal_cells() {
    let root = std::env::temp_dir().join(format!("tf-bead-2-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut image = image::RgbaImage::new(16, 16);
    for (x, _y, pixel) in image.enumerate_pixels_mut() {
        *pixel = if x < 8 {
            image::Rgba([0, 0, 255, 255])
        } else {
            image::Rgba([255, 215, 0, 255])
        };
    }
    let path = root.join("split.png");
    image.save(&path).unwrap();
    let out = generate(Args {
        path: path.to_string_lossy().into_owned(),
        columns: 8,
        rows: 8,
        max_colors: Some(2),
        keep_transparent: false,
    })
    .unwrap();
    assert_eq!(
        out.total_beads,
        out.cells.iter().filter(|c| !c.color.is_empty()).count()
    );
    assert_eq!(out.usage.len(), 2, "max_colors 2 constrains the palette");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn validates_grid_and_missing_file() {
    let err = generate(Args {
        path: "/nonexistent/x.png".into(),
        columns: 3,
        rows: 8,
        max_colors: None,
        keep_transparent: false,
    })
    .unwrap_err();
    assert!(err.code.starts_with("bead."), "unexpected {}", err.code);
    let err = generate(Args {
        path: "/nonexistent/x.png".into(),
        columns: 200,
        rows: 8,
        max_colors: None,
        keep_transparent: false,
    })
    .unwrap_err();
    assert_eq!(err.code, "bead.bad_grid");
}

#[test]
fn dispatches_via_plugin() {
    assert_eq!(
        BeadPattern::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&BeadPattern::default()).unwrap();
    assert_eq!(checked, 1);
}
