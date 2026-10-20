use super::*;
use image::Rgba;

fn make_work(width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_fn(width, height, |x, y| Rgba([(x * 7 % 256) as u8, (y * 5 % 256) as u8, 128, 255]))
}

#[test]
fn renders_png_with_expected_size_and_stroke() {
    let work = make_work(64, 48);
    let points = [[0.0, 0.0], [63.0, 0.0], [63.0, 47.0], [0.0, 47.0], [31.5, 23.0]];
    let triangles = delaunay::triangulate(&points);
    let colors = sample::colors(&work, &points, &triangles, 0);
    let bytes = render_png(&work, &points, &triangles, &colors, 2.0, 2, [17, 17, 17], [255, 255, 255]).unwrap();
    // PNG 魔数
    assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']));
    let decoded = image::load_from_memory(&bytes).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (128, 96));
    // 边缘应是描边色：输出 (0,0) 在边线上
    let pixel = decoded.to_rgba8().get_pixel(0, 0).0;
    assert_eq!(&pixel[..3], &[17, 17, 17]);
}

#[test]
fn renders_svg_polygons_not_embedded_bitmaps() {
    let work = make_work(64, 48);
    let points = [[0.0, 0.0], [63.0, 0.0], [63.0, 47.0], [0.0, 47.0], [31.5, 23.0]];
    let triangles = delaunay::triangulate(&points);
    let colors = sample::colors(&work, &points, &triangles, 0);
    let svg = render_svg(&work, &points, &triangles, &colors, 1.0, 1, "111111", "#ffffff");
    assert!(svg.starts_with("<?xml"));
    assert!(svg.contains(r#"<polygon points=""#));
    assert_eq!(svg.matches("<polygon").count(), triangles.len());
    assert!(!svg.contains("base64"), "SVG must be real vectors");
    assert!(svg.contains(r#"fill="#""#) || svg.contains(r#"fill="#"#));
    assert!(svg.contains(r#"stroke-width="1""#));
}
