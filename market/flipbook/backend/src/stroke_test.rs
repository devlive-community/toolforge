use super::*;

fn brush(points: Vec<[f32; 2]>, size: f32) -> Stroke {
    Stroke {
        tool: Tool::Brush,
        color: "#ff0000".into(),
        size,
        points,
    }
}

#[test]
fn a_line_paints_along_its_path_only() {
    let mut image = RgbaImage::new(40, 20);
    apply(&mut image, &brush(vec![[2.0, 10.0], [38.0, 10.0]], 4.0)).unwrap();
    assert_eq!(image.get_pixel(20, 10)[0], 255);
    assert_eq!(image.get_pixel(20, 10)[3], 255);
    assert_eq!(image.get_pixel(20, 2)[3], 0, "远离笔画的像素不受影响");
}

#[test]
fn overlapping_stamps_do_not_darken_soft_edges() {
    // 同一笔来回经过同一位置，边缘半透明像素保持一次覆盖的透明度
    let mut once = RgbaImage::new(40, 20);
    apply(&mut once, &brush(vec![[5.0, 10.0], [35.0, 10.0]], 5.0)).unwrap();
    let mut twice = RgbaImage::new(40, 20);
    apply(
        &mut twice,
        &brush(vec![[5.0, 10.0], [35.0, 10.0], [5.0, 10.0]], 5.0),
    )
    .unwrap();
    assert_eq!(once, twice);
}

#[test]
fn a_single_tap_leaves_a_dot() {
    let mut image = RgbaImage::new(10, 10);
    apply(&mut image, &brush(vec![[5.0, 5.0]], 3.0)).unwrap();
    assert_eq!(image.get_pixel(5, 5)[3], 255);
}

#[test]
fn eraser_clears_alpha() {
    let mut image = RgbaImage::from_pixel(20, 20, image::Rgba([0, 0, 255, 255]));
    let eraser = Stroke {
        tool: Tool::Eraser,
        color: String::new(),
        size: 6.0,
        points: vec![[10.0, 10.0]],
    };
    apply(&mut image, &eraser).unwrap();
    assert_eq!(image.get_pixel(10, 10)[3], 0);
    assert_eq!(image.get_pixel(0, 0)[3], 255);
}

#[test]
fn strokes_outside_the_canvas_are_clipped_safely() {
    let mut image = RgbaImage::new(10, 10);
    apply(
        &mut image,
        &brush(vec![[-50.0, -50.0], [-20.0, 400.0]], 8.0),
    )
    .unwrap();
    assert!(image.pixels().all(|p| p[3] == 0));
}

#[test]
fn invalid_strokes_are_rejected() {
    let mut image = RgbaImage::new(10, 10);
    assert!(apply(&mut image, &brush(vec![], 3.0)).is_err());
    assert!(apply(&mut image, &brush(vec![[1.0, 1.0]], 200.0)).is_err());
    assert!(apply(&mut image, &brush(vec![[f32::NAN, 1.0]], 3.0)).is_err());
    let mut bad = brush(vec![[1.0, 1.0]], 3.0);
    bad.color = "blue".into();
    assert!(apply(&mut image, &bad).is_err());
}

#[test]
fn huge_coordinates_finish_quickly() {
    let mut image = RgbaImage::new(10, 10);
    let start = std::time::Instant::now();
    apply(&mut image, &brush(vec![[-1.0e9, 5.5], [1.0e9, 5.5]], 1.0)).unwrap();
    assert!(start.elapsed().as_secs() < 2);
    assert_eq!(image.get_pixel(5, 5)[3], 255, "穿过画布的部分仍然画出");
}
