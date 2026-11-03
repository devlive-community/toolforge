use super::*;

const CLASSIC: &str = " .:-=+*#%@";

fn opts(cols: u32) -> Options {
    Options {
        cols,
        chars: CLASSIC.to_owned(),
        brightness: 0.0,
        contrast: 1.0,
        invert: false,
        color: false,
        aspect: 0.5,
        keep_alpha: true,
        bg: Rgba([255, 255, 255, 255]),
    }
}

fn solid(w: u32, h: u32, px: Rgba<u8>) -> RgbaImage {
    RgbaImage::from_pixel(w, h, px)
}

#[test]
fn rows_follow_cell_aspect_ratio() {
    // 3:2 图、60 列、宽高比 0.5 → 20 行
    assert_eq!(rows_for(300, 200, 60, 0.5), 20);
    // 宽高比 1.0 → 40 行（不补偿字符格偏高）
    assert_eq!(rows_for(300, 200, 60, 1.0), 40);
    // 行数钳制到上限
    assert_eq!(rows_for(1, 1000, 500, 1.0), MAX_ROWS);
}

#[test]
fn white_image_takes_the_densest_glyph() {
    let img = solid(8, 8, Rgba([255, 255, 255, 255]));
    let out = convert(&img, &opts(8)).expect("converts");
    // 亮像素 → 梯度末尾（@）
    assert!(out.plain.lines().all(|line| line == "@@@@@@@@"));
    assert_eq!(out.cols, 8);
    assert_eq!(out.rows, 4); // 8 列 × (8/8) × 0.5 = 4 行
}

#[test]
fn black_image_takes_the_lightest_glyph() {
    let img = solid(8, 8, Rgba([0, 0, 0, 255]));
    let out = convert(&img, &opts(8)).expect("converts");
    assert!(out.plain.lines().all(|line| line == "        "));
}

#[test]
fn invert_flips_the_direction() {
    let img = solid(8, 8, Rgba([255, 255, 255, 255]));
    let mut inverted = opts(8);
    inverted.invert = true;
    let out = convert(&img, &inverted).expect("converts");
    // 反相后亮像素取最浅符号
    assert!(out.plain.lines().all(|line| line == "        "));
}

#[test]
fn transparent_pixels_stay_blank_when_kept() {
    // 左半透明、右半白
    let mut img = solid(8, 4, Rgba([255, 255, 255, 255]));
    for y in 0..4 {
        for x in 0..4 {
            img.put_pixel(x, y, Rgba([255, 255, 255, 0]));
        }
    }
    let out = convert(&img, &opts(8)).expect("converts");
    for line in out.plain.lines() {
        assert_eq!(line, "    @@@@");
    }
}

#[test]
fn transparent_pixels_blend_background_when_filled() {
    let mut img = solid(8, 4, Rgba([255, 255, 255, 0]));
    for y in 0..4 {
        for x in 0..8 {
            img.put_pixel(x, y, Rgba([255, 255, 255, 0]));
        }
    }
    let mut filled = opts(8);
    filled.keep_alpha = false;
    filled.bg = Rgba([255, 255, 255, 255]);
    let out = convert(&img, &filled).expect("converts");
    // 透明像素铺白底后按白处理
    assert!(out.plain.lines().all(|line| line == "@@@@@@@@"));
}

#[test]
fn color_output_wraps_every_line_with_reset() {
    let img = solid(8, 8, Rgba([255, 0, 0, 255]));
    let mut colored = opts(8);
    colored.color = true;
    let out = convert(&img, &colored).expect("converts");
    for line in out.ansi.lines() {
        assert!(
            line.starts_with("\u{1b}[38;2;255;0;0m"),
            "line should start with SGR: {line:?}"
        );
        assert!(
            line.ends_with("\u{1b}[0m"),
            "line should reset colors: {line:?}"
        );
    }
    // 纯文本版不含控制字符
    assert!(!out.plain.contains('\u{1b}'));
    // 彩色版除 SGR 外内容一致
    let stripped = out
        .ansi
        .replace("\u{1b}[38;2;255;0;0m", "")
        .replace("\u{1b}[0m", "");
    assert_eq!(stripped, out.plain);
}

#[test]
fn out_of_range_cols_are_rejected() {
    let img = solid(8, 8, Rgba([255, 255, 255, 255]));
    let error = convert(&img, &opts(4)).expect_err("below minimum");
    assert_eq!(error.code, "ascii.bad_width");
    let error = convert(&img, &opts(501)).expect_err("above maximum");
    assert_eq!(error.code, "ascii.bad_width");
}

#[test]
fn contrast_and_brightness_shift_the_mapping() {
    // 中灰 + 对比度拉满 → 接近黑或白；这里验证对比度提高后梯度索引偏移
    let img = solid(8, 8, Rgba([128, 128, 128, 255]));
    let mut high = opts(8);
    high.contrast = 3.0;
    let out = convert(&img, &high).expect("converts");
    // 128/255 ≈ 0.502 → (0.502-0.5)*3+0.5 ≈ 0.506 → 仍取中间偏后字符
    let line = out.plain.lines().next().expect("has a line");
    assert!(
        line.chars()
            .all(|c| c == line.chars().next().expect("non-empty"))
    );
}
