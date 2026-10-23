//! 测试用图片：带 EXIF 的 JPEG 与带文本 / EXIF 块的 PNG
use std::io::Cursor;

use exif::experimental::Writer;
use exif::{Field, In, Rational, Tag, Value};
use image::codecs::jpeg::JpegEncoder;
use image::{ImageFormat, Rgb, RgbImage};

pub fn image() -> RgbImage {
    RgbImage::from_fn(64, 48, |x, _| {
        if x < 32 {
            Rgb([220, 30, 30])
        } else {
            Rgb([20, 60, 200])
        }
    })
}

fn exif_blob() -> Vec<u8> {
    let make = Field {
        tag: Tag::Make,
        ifd_num: In::PRIMARY,
        value: Value::Ascii(vec![b"ToolForge".to_vec()]),
    };
    let model = Field {
        tag: Tag::Model,
        ifd_num: In::PRIMARY,
        value: Value::Ascii(vec![b"Test Cam".to_vec()]),
    };
    let orientation = Field {
        tag: Tag::Orientation,
        ifd_num: In::PRIMARY,
        value: Value::Short(vec![6]),
    };
    let lat_ref = Field {
        tag: Tag::GPSLatitudeRef,
        ifd_num: In::PRIMARY,
        value: Value::Ascii(vec![b"S".to_vec()]),
    };
    let lat = Field {
        tag: Tag::GPSLatitude,
        ifd_num: In::PRIMARY,
        value: Value::Rational(vec![
            Rational { num: 33, denom: 1 },
            Rational { num: 51, denom: 1 },
            Rational { num: 36, denom: 1 },
        ]),
    };
    let lon_ref = Field {
        tag: Tag::GPSLongitudeRef,
        ifd_num: In::PRIMARY,
        value: Value::Ascii(vec![b"E".to_vec()]),
    };
    let lon = Field {
        tag: Tag::GPSLongitude,
        ifd_num: In::PRIMARY,
        value: Value::Rational(vec![
            Rational { num: 151, denom: 1 },
            Rational { num: 12, denom: 1 },
            Rational { num: 36, denom: 1 },
        ]),
    };
    let alt = Field {
        tag: Tag::GPSAltitude,
        ifd_num: In::PRIMARY,
        value: Value::Rational(vec![Rational {
            num: 125,
            denom: 10,
        }]),
    };
    let mut writer = Writer::new();
    for field in [
        &make,
        &model,
        &orientation,
        &lat_ref,
        &lat,
        &lon_ref,
        &lon,
        &alt,
    ] {
        writer.push_field(field);
    }
    let mut buffer = Cursor::new(Vec::new());
    writer.write(&mut buffer, false).unwrap();
    buffer.into_inner()
}

/// 在 SOI 之后插入 APP1（EXIF）段与一个注释段
pub fn jpeg_with_exif() -> Vec<u8> {
    let mut plain = Vec::new();
    JpegEncoder::new_with_quality(&mut plain, 90)
        .encode_image(&image())
        .unwrap();
    let mut app1 = b"Exif\0\0".to_vec();
    app1.extend(exif_blob());
    let mut out = plain[..2].to_vec();
    out.extend([0xFF, 0xE1]);
    out.extend(((app1.len() + 2) as u16).to_be_bytes());
    out.extend(&app1);
    let comment = b"secret comment";
    out.extend([0xFF, 0xFE]);
    out.extend(((comment.len() + 2) as u16).to_be_bytes());
    out.extend(comment);
    out.extend(&plain[2..]);
    out
}

fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = (data.len() as u32).to_be_bytes().to_vec();
    out.extend(kind);
    out.extend(data);
    // 读取器不校验 CRC，这里写 0 即可
    out.extend([0, 0, 0, 0]);
    out
}

/// 在 IHDR 之后插入 tEXt 与 eXIf 块
pub fn png_with_text() -> Vec<u8> {
    let mut plain = Vec::new();
    image()
        .write_to(&mut Cursor::new(&mut plain), ImageFormat::Png)
        .unwrap();
    let ihdr_end = 8 + 12 + 13;
    let mut out = plain[..ihdr_end].to_vec();
    out.extend(chunk(b"tEXt", b"Author\0Ada"));
    out.extend(chunk(b"eXIf", &exif_blob()));
    out.extend(&plain[ihdr_end..]);
    out
}
