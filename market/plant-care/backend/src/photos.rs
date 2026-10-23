use crate::model::*;
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageDecoder, ImageReader, RgbImage};
use std::io::{Cursor, Read};
use tf_plugin_api::{PluginResult, TaskContext};
const PREFIX: &str = "data:image/jpeg;base64,";
fn encode(image: &DynamicImage, quality: u8) -> PluginResult<Vec<u8>> {
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, quality)
        .encode_image(image)
        .map_err(|_| error("photo"))?;
    Ok(out)
}
fn uri(bytes: &[u8]) -> String {
    format!("{PREFIX}{}", data_encoding::BASE64.encode(bytes))
}
fn decode(value: &str, max: usize) -> PluginResult<Vec<u8>> {
    if value.len() > PREFIX.len() + max.div_ceil(3) * 4 {
        return Err(error("photo"));
    }
    let data = data_encoding::BASE64
        .decode(
            value
                .strip_prefix(PREFIX)
                .ok_or_else(|| error("photo"))?
                .as_bytes(),
        )
        .map_err(|_| error("photo"))?;
    if data.len() > max || !data.starts_with(&[0xff, 0xd8]) {
        return Err(error("photo"));
    }
    Ok(data)
}
pub fn validate(p: &Photo) -> PluginResult<()> {
    let bytes = decode(&p.data, 49152)?;
    let thumbnail = decode(&p.thumbnail, 8192)?;
    let (tw, th) = ImageReader::new(Cursor::new(&thumbnail))
        .with_guessed_format()
        .map_err(|_| error("photo"))?
        .into_dimensions()
        .map_err(|_| error("photo"))?;
    if tw == 0 || th == 0 || tw > 96 || th > 96 {
        return Err(error("photo"));
    }
    image::load_from_memory_with_format(&thumbnail, image::ImageFormat::Jpeg)
        .map_err(|_| error("photo"))?;
    let reader = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|_| error("photo"))?;
    let (w, h) = reader.into_dimensions().map_err(|_| error("photo"))?;
    if bytes.len() != p.bytes
        || w != p.width
        || h != p.height
        || w == 0
        || h == 0
        || w > 960
        || h > 960
    {
        return Err(error("photo"));
    }
    image::load_from_memory_with_format(&bytes, image::ImageFormat::Jpeg)
        .map_err(|_| error("photo"))?;
    Ok(())
}
pub fn import(
    id: String,
    plant_id: String,
    date: String,
    note: String,
    path: &str,
    ctx: &dyn TaskContext,
) -> PluginResult<Photo> {
    ctx.stage("readPhoto");
    if path.is_empty() || path.len() > 4096 || ctx.file_size(path)? > 16 * 1024 * 1024 {
        return Err(error("photo_size"));
    }
    let mut bytes = Vec::new();
    let mut reader = ctx.open_file(path)?.take(16 * 1024 * 1024 + 1);
    let mut chunk = [0; 65536];
    loop {
        check(ctx)?;
        let n = reader.read(&mut chunk).map_err(|_| error("photo"))?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..n]);
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(error("photo_size"));
        }
    }
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| error("photo"))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8000);
    limits.max_image_height = Some(8000);
    limits.max_alloc = Some(192 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|_| error("photo"))?;
    let (w, h) = decoder.dimensions();
    if u64::from(w) * u64::from(h) > 24_000_000 {
        return Err(error("pixels"));
    }
    let orientation = decoder.orientation().map_err(|_| error("photo"))?;
    ctx.stage("compressPhoto");
    let mut image = DynamicImage::from_decoder(decoder).map_err(|_| error("photo"))?;
    image.apply_orientation(orientation);
    check(ctx)?;
    // Normalize before persistence: no original path or EXIF metadata is retained.
    if image.width() > 960 || image.height() > 960 {
        image = image.thumbnail(960, 960);
    }
    let rgba = image.into_rgba8();
    let mut flat = RgbImage::new(rgba.width(), rgba.height());
    for (target, source) in flat.pixels_mut().zip(rgba.pixels()) {
        let a = u16::from(source[3]);
        for i in 0..3 {
            target[i] = ((u16::from(source[i]) * a + 255 * (255 - a) + 127) / 255) as u8;
        }
    }
    let mut image = DynamicImage::ImageRgb8(flat);
    let mut encoded = Vec::new();
    for edge in [960, 768, 640, 512, 384, 256] {
        check(ctx)?;
        if image.width() > edge || image.height() > edge {
            image = image.thumbnail(edge, edge);
        }
        encoded = encode(&image, 72)?;
        if encoded.len() <= 49152 {
            break;
        }
    }
    if encoded.len() > 49152 {
        return Err(error("photo_size"));
    }
    let thumb = if image.width() > 96 || image.height() > 96 {
        image.thumbnail(96, 96)
    } else {
        image.clone()
    };
    let thumbnail = encode(&thumb, 60)?;
    if thumbnail.len() > 8192 {
        return Err(error("photo_size"));
    }
    Ok(Photo {
        id,
        plant_id,
        date,
        note,
        data: uri(&encoded),
        thumbnail: uri(&thumbnail),
        width: image.width(),
        height: image.height(),
        bytes: encoded.len(),
    })
}
