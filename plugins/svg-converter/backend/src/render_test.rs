use super::*;

/// 一个 24×24 的蓝色方块，右下半透明
const SAMPLE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><rect width="12" height="12" fill="#0000ff"/><rect x="12" y="12" width="12" height="12" fill="#0000ff" fill-opacity="0.5"/></svg>"##;

/// 只有 viewBox 没有 width / height 的 SVG，尺寸取自 viewBox
const VIEWBOX_ONLY: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 20"><rect width="40" height="20" fill="#00ff00"/></svg>"##;

#[test]
fn reads_the_declared_size() {
    let tree = parse(SAMPLE.as_bytes()).unwrap();
    let size = tree.size();
    assert_eq!((size.width(), size.height()), (24.0, 24.0));
}

#[test]
fn falls_back_to_the_viewbox_size() {
    let tree = parse(VIEWBOX_ONLY.as_bytes()).unwrap();
    let size = tree.size();
    assert_eq!((size.width(), size.height()), (40.0, 20.0));
}

#[test]
fn rejects_broken_files() {
    let err = parse(b"not an svg").unwrap_err();
    assert_eq!(err.code, "svg.invalid");
}

#[test]
fn renders_the_declared_size() {
    let tree = parse(SAMPLE.as_bytes()).unwrap();
    let image = render(&tree, 24, 24).unwrap();
    assert_eq!(image.dimensions(), (24, 24));
    // 左上角是不透明纯蓝
    assert_eq!(image.get_pixel(0, 0).0, [0, 0, 255, 255]);
}

#[test]
fn renders_transparent_areas() {
    let tree = parse(SAMPLE.as_bytes()).unwrap();
    let image = render(&tree, 24, 24).unwrap();
    // 右上角在方块之外，应为全透明
    assert_eq!(image.get_pixel(23, 0).0, [0, 0, 0, 0]);
}

#[test]
fn renders_scaled_sizes() {
    let tree = parse(SAMPLE.as_bytes()).unwrap();
    let image = render(&tree, 48, 48).unwrap();
    assert_eq!(image.dimensions(), (48, 48));
    assert_eq!(image.get_pixel(0, 0).0, [0, 0, 255, 255]);
}

#[test]
fn rejects_zero_sized_targets() {
    let tree = parse(SAMPLE.as_bytes()).unwrap();
    assert_eq!(render(&tree, 0, 10).unwrap_err().code, "svg.render_failed");
}
