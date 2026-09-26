use image::{Rgb, RgbImage, Rgba, RgbaImage};

use super::*;

/// 带渐变与噪点的类照片图像
fn photo(width: u32, height: u32) -> DynamicImage {
    let mut seed = 7u32;
    DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |x, y| {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let noise = (seed >> 24) as u8 / 16;
        Rgb([
            ((x * 240 / width) as u8).saturating_add(noise),
            (y * 255 / height) as u8,
            ((x + y) * 127 / (width + height)) as u8 + noise,
        ])
    }))
}

fn logo() -> DynamicImage {
    DynamicImage::ImageRgba8(RgbaImage::from_fn(64, 48, |x, y| {
        if x < 16 {
            Rgba([0, 0, 0, 0])
        } else if y < 24 {
            Rgba([220, 40, 40, 255])
        } else {
            Rgba([20, 90, 200, (x * 4) as u8])
        }
    }))
}

fn png_bytes(image: &DynamicImage) -> Vec<u8> {
    let mut buffer = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut buffer), ImageFormat::Png)
        .unwrap();
    buffer
}

/// 最小的 EXIF（TIFF 小端），只含方向标签
fn exif_orientation(value: u16) -> Vec<u8> {
    let mut tiff = b"II*\0\x08\0\0\0".to_vec();
    tiff.extend(1u16.to_le_bytes());
    tiff.extend(0x0112u16.to_le_bytes());
    tiff.extend(3u16.to_le_bytes());
    tiff.extend(1u32.to_le_bytes());
    tiff.extend(value.to_le_bytes());
    tiff.extend([0, 0, 0, 0, 0, 0]);
    tiff
}

/// 解码器只原样读出配置文件内容，测试不需要真实的 ICC 数据
const ICC: &[u8] = b"toolforge test icc profile";

#[test]
fn fits_the_long_edge() {
    assert_eq!(fit(4000, 3000, 2000), (2000, 1500));
    assert_eq!(fit(3000, 4000, 2000), (1500, 2000));
    assert_eq!(fit(800, 600, 2000), (800, 600));
    assert_eq!(fit(800, 600, 0), (800, 600));
    assert_eq!(fit(10_000, 1, 100), (100, 1));
}

#[test]
fn packs_low_bit_depths() {
    assert_eq!(
        pack(&[1, 0, 1, 1, 0, 0, 0, 0, 1], 9, png::BitDepth::One),
        vec![0b1011_0000, 0b1000_0000]
    );
    assert_eq!(
        pack(&[3, 2, 1, 0, 1], 5, png::BitDepth::Two),
        vec![0b1110_0100, 0b0100_0000]
    );
    assert_eq!(pack(&[15, 1, 2], 3, png::BitDepth::Four), vec![0xf1, 0x20]);
    assert_eq!(
        pack(&[1, 2, 3, 4], 2, png::BitDepth::Four),
        vec![0x12, 0x34]
    );
}

#[test]
fn compresses_jpeg_progressively_and_keeps_color_profiles() {
    let jpeg = encode_jpeg(&photo(320, 240), 95, Some(ICC), None).unwrap();
    let smaller = compress(
        &jpeg,
        &Options {
            quality: 60,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(smaller.format, Format::Jpeg);
    assert!(
        smaller.bytes.len() < jpeg.len(),
        "{} >= {}",
        smaller.bytes.len(),
        jpeg.len()
    );
    // SOF2 = 渐进式
    assert!(smaller.bytes.windows(2).any(|w| w == [0xff, 0xc2]));
    let decoded = decode(&smaller.bytes).unwrap();
    assert_eq!((decoded.image.width(), decoded.image.height()), (320, 240));
    assert_eq!(decoded.icc.as_deref(), Some(ICC));
    assert!(decoded.exif.is_none());
}

#[test]
fn applies_orientation_unless_exif_is_kept() {
    let jpeg = encode_jpeg(&photo(40, 20), 90, None, Some(&exif_orientation(6))).unwrap();
    assert_eq!(decode(&jpeg).unwrap().orientation, Orientation::Rotate90);

    let stripped = compress(&jpeg, &Options::default()).unwrap();
    let decoded = decode(&stripped.bytes).unwrap();
    assert_eq!((decoded.image.width(), decoded.image.height()), (20, 40));
    assert!(decoded.exif.is_none());

    let kept = compress(
        &jpeg,
        &Options {
            keep_metadata: true,
            ..Options::default()
        },
    )
    .unwrap();
    let decoded = decode(&kept.bytes).unwrap();
    assert_eq!((decoded.image.width(), decoded.image.height()), (40, 20));
    assert_eq!(decoded.orientation, Orientation::Rotate90);
}

#[test]
fn quantizes_png_and_preserves_transparency() {
    let original = png_bytes(&logo());
    let out = compress(
        &original,
        &Options {
            colors: 16,
            ..Options::default()
        },
    )
    .unwrap();
    let decoder = png::Decoder::new(Cursor::new(&out.bytes));
    let reader = decoder.read_info().unwrap();
    let info = reader.info();
    assert_eq!(info.color_type, png::ColorType::Indexed);
    assert!(info.palette.as_ref().unwrap().len() / 3 <= 16);
    let decoded = decode(&out.bytes).unwrap().image.to_rgba8();
    assert_eq!(
        decoded.get_pixel(0, 0)[3],
        0,
        "transparent stays transparent"
    );
    let red = decoded.get_pixel(40, 10);
    assert!(red[0] > 200 && red[1] < 60 && red[3] == 255, "{red:?}");
}

#[test]
fn keeps_png_pixels_in_lossless_mode() {
    let image = photo(64, 64);
    let original = png_bytes(&image);
    let options = Options {
        png_mode: PngMode::Lossless,
        ..Options::default()
    };
    let out = compress(&original, &options).unwrap();
    assert_eq!(decode(&out.bytes).unwrap().image.to_rgb8(), image.to_rgb8());
    // 缩放后重新编码同样无损
    let resized = compress(
        &original,
        &Options {
            max_size: 32,
            ..options
        },
    )
    .unwrap();
    assert_eq!((resized.width, resized.height), (32, 32));
}

#[test]
fn converts_to_webp_and_jpeg() {
    let original = png_bytes(&logo());
    let webp = compress(
        &original,
        &Options {
            format: Format::Webp,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(webp.format, Format::Webp);
    let decoded = decode(&webp.bytes).unwrap();
    assert_eq!(decoded.format, Format::Webp);
    assert!(decoded.image.color().has_alpha());

    let jpeg = compress(
        &original,
        &Options {
            format: Format::Jpeg,
            ..Options::default()
        },
    )
    .unwrap();
    let pixel = decode(&jpeg.bytes)
        .unwrap()
        .image
        .to_rgb8()
        .get_pixel(2, 2)
        .0;
    assert!(
        pixel.iter().all(|c| *c > 240),
        "transparent areas become white: {pixel:?}"
    );
}

#[test]
fn compresses_grayscale_jpeg() {
    let gray = DynamicImage::ImageLuma8(image::GrayImage::from_fn(50, 30, |x, _| {
        image::Luma([(x * 5) as u8])
    }));
    let jpeg = encode_jpeg(&gray, 80, None, None).unwrap();
    assert!(decode(&jpeg).unwrap().image.color().channel_count() == 1);
}

#[test]
fn rejects_unsupported_and_broken_files() {
    assert_eq!(
        compress(b"BM\0\0\0\0", &Options::default())
            .unwrap_err()
            .code,
        "image.unsupported"
    );
    assert_eq!(
        compress(b"GIF89a....", &Options::default())
            .unwrap_err()
            .code,
        "image.unsupported"
    );
    let mut broken = png_bytes(&logo());
    broken.truncate(60);
    assert_eq!(
        compress(&broken, &Options::default()).unwrap_err().code,
        "image.decode_failed"
    );
}

#[test]
fn splits_large_icc_profiles_from_sequence_one() {
    let big = vec![7u8; 150_000];
    let markers = icc_markers(&big);
    assert_eq!(markers.len(), 3);
    for (index, marker) in markers.iter().enumerate() {
        assert!(marker.starts_with(b"ICC_PROFILE\0"));
        assert_eq!((marker[12], marker[13]), (index as u8 + 1, 3));
        assert!(marker.len() <= 65_533);
    }
    let jpeg = encode_jpeg(&photo(16, 16), 80, Some(&big), None).unwrap();
    assert_eq!(decode(&jpeg).unwrap().icc, Some(big));
    assert!(icc_markers(&[]).is_empty());
}

/// `TOOLFORGE_COMPRESS_SAMPLES=<目录> cargo test --release -p tfp-image-compressor -- --ignored --nocapture`
#[test]
#[ignore]
fn benchmark_real_images() {
    let Some(dir) = std::env::var_os("TOOLFORGE_COMPRESS_SAMPLES") else {
        return;
    };
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let bytes = std::fs::read(entry.path()).unwrap();
        for (label, options) in [
            ("keep", Options::default()),
            (
                "webp",
                Options {
                    format: Format::Webp,
                    ..Options::default()
                },
            ),
            (
                "lossless",
                Options {
                    png_mode: PngMode::Lossless,
                    ..Options::default()
                },
            ),
        ] {
            let start = std::time::Instant::now();
            match compress(&bytes, &options) {
                Ok(out) => {
                    if let Some(dir) = std::env::var_os("TOOLFORGE_COMPRESS_OUT") {
                        let name = format!(
                            "{}-{label}.{}",
                            entry.file_name().to_string_lossy(),
                            out.format.extension()
                        );
                        std::fs::write(std::path::Path::new(&dir).join(name), &out.bytes).unwrap();
                    }
                    println!(
                        "{:?} {label}: {} -> {} ({:.0}%) {}x{} in {} ms",
                        entry.file_name(),
                        bytes.len(),
                        out.bytes.len(),
                        100.0 * out.bytes.len() as f64 / bytes.len() as f64,
                        out.width,
                        out.height,
                        start.elapsed().as_millis()
                    )
                }
                Err(err) => println!("{:?} {label}: {}", entry.file_name(), err.code),
            }
        }
    }
}

#[test]
fn keeps_opaque_png_free_of_transparency() {
    let out = compress(&png_bytes(&photo(120, 80)), &Options::default()).unwrap();
    let reader = png::Decoder::new(Cursor::new(&out.bytes))
        .read_info()
        .unwrap();
    assert_eq!(reader.info().color_type, png::ColorType::Indexed);
    assert!(reader.info().trns.is_none());
    let decoded = decode(&out.bytes).unwrap().image.to_rgba8();
    assert!(decoded.pixels().all(|p| p[3] == 255));
}
