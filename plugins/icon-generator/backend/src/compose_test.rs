use super::*;

fn red(w: u32, h: u32) -> RgbaImage {
    RgbaImage::from_pixel(w, h, Rgba([255, 0, 0, 255]))
}

#[test]
fn parses_colors() {
    assert_eq!(parse_color("#1e90ff"), Some([0x1e, 0x90, 0xff]));
    assert_eq!(parse_color("fff"), Some([255, 255, 255]));
    assert_eq!(parse_color("#12345"), None);
    let style: Style = serde_json::from_value(serde_json::json!({ "background": "" })).unwrap();
    assert_eq!(style.background, None);
    assert!(serde_json::from_value::<Style>(serde_json::json!({ "background": "blue" })).is_err());
    assert_eq!(
        Style {
            padding: 0.5,
            ..Style::default()
        }
        .validate()
        .unwrap_err()
        .code,
        "icon.invalid_style"
    );
}

#[test]
fn fits_wide_images_with_padding() {
    let style = Style {
        padding: 0.25,
        ..Style::default()
    };
    let out = master(&red(200, 100), &style, Shape::Rounded, 100);
    // 内容区 50×50，宽图缩放为 50×25 并居中
    assert_eq!(out.get_pixel(50, 50).0, [255, 0, 0, 255]);
    assert_eq!(out.get_pixel(30, 50).0, [255, 0, 0, 255]);
    assert_eq!(out.get_pixel(20, 50).0[3], 0);
    assert_eq!(out.get_pixel(50, 35).0[3], 0);
}

#[test]
fn rounds_corners_and_fills_the_background() {
    let style = Style {
        background: Some([0, 0, 255]),
        radius: 0.25,
        ..Style::default()
    };
    let out = master(
        &red(10, 10),
        &Style {
            padding: 0.3,
            ..style
        },
        Shape::Rounded,
        100,
    );
    assert_eq!(out.get_pixel(0, 0).0[3], 0, "corner is cut");
    assert_eq!(
        out.get_pixel(50, 2).0,
        [0, 0, 255, 255],
        "edge has background"
    );
    assert_eq!(out.get_pixel(50, 50).0, [255, 0, 0, 255]);
    // 圆角边缘抗锯齿
    let edge = (0..30)
        .map(|i| out.get_pixel(i, i).0[3])
        .collect::<Vec<_>>();
    assert!(edge.iter().any(|a| *a > 0 && *a < 255), "{edge:?}");
}

#[test]
fn opaque_shapes_never_keep_transparency() {
    let clear = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 0, 0]));
    let style = Style {
        radius: 0.5,
        ..Style::default()
    };
    let out = master(&clear, &style, Shape::Opaque, 64);
    assert!(out.pixels().all(|p| p.0 == [255, 255, 255, 255]));
}

#[test]
fn circles_and_mac_tiles_leave_the_corners_empty() {
    let style = Style::default();
    let circle = master(&red(10, 10), &style, Shape::Circle, 100);
    assert_eq!(circle.get_pixel(2, 2).0[3], 0);
    // 圆形图标的内容至少留 15% 边距，四角不会被裁掉
    assert_eq!(circle.get_pixel(10, 50).0, [255, 255, 255, 255]);
    assert_eq!(circle.get_pixel(50, 50).0, [255, 0, 0, 255]);

    let mac = master(&red(10, 10), &style, Shape::Mac, 1024);
    assert_eq!(mac.get_pixel(50, 512).0[3], 0, "shadow margin");
    assert_eq!(mac.get_pixel(512, 512).0, [255, 0, 0, 255]);
    assert_eq!(mac.get_pixel(110, 110).0[3], 0, "rounded tile corner");
}

#[test]
fn scales_to_each_size() {
    let out = master(&red(10, 10), &Style::default(), Shape::Rounded, MASTER);
    assert_eq!(scaled(&out, 16).dimensions(), (16, 16));
    assert_eq!(scaled(&out, MASTER).dimensions(), (MASTER, MASTER));
}
