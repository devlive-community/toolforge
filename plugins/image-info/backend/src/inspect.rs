use std::io::Cursor;
use std::path::{Path, PathBuf};

use exif::{Exif, In, Tag, Value};
use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::palette::{Swatch, extract};
use crate::strip::strip;

const MAX_FILE: u64 = 200 * 1024 * 1024;
const PREVIEW: u32 = 640;

#[derive(Deserialize)]
pub struct Args {
    path: String,
}

#[derive(Debug, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Gps {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
}

/// 常用 EXIF 字段（值为 EXIF 规范的显示文本，属于数据而非界面文案）
#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub make: Option<String>,
    pub model: Option<String>,
    pub lens: Option<String>,
    pub taken_at: Option<String>,
    pub exposure: Option<String>,
    pub aperture: Option<String>,
    pub iso: Option<String>,
    pub focal_length: Option<String>,
    pub flash: Option<String>,
    pub software: Option<String>,
    pub orientation: Option<u32>,
    pub gps: Option<Gps>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub ifd: String,
    pub tag: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub format: Option<String>,
    pub width: u32,
    pub height: u32,
    /// 约分后的宽高比，如 [16, 9]
    pub aspect: [u32; 2],
    pub color_type: String,
    pub has_alpha: bool,
    pub bits_per_pixel: u16,
    pub preview: Option<String>,
    pub palette: Vec<Swatch>,
    pub average: Option<String>,
    pub summary: Option<Summary>,
    pub exif: Vec<Entry>,
    /// JPEG / PNG 可无损移除元数据
    pub can_strip: bool,
}

#[derive(Deserialize)]
pub struct StripArgs {
    path: String,
    /// 保存位置；为空时写到源文件旁边
    #[serde(default)]
    output: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StripReport {
    pub output: String,
    pub removed: Vec<String>,
    pub saved_bytes: u64,
    /// 原图带有非默认方向信息，移除后可能显示为旋转前的方向
    pub orientation_lost: bool,
}

fn io(err: std::io::Error, path: &str) -> PluginError {
    let code = if err.kind() == std::io::ErrorKind::NotFound {
        "fs.not_found"
    } else {
        "fs.io"
    };
    PluginError::new(code)
        .with("path", path)
        .with("detail", err.to_string())
}

fn read(path: &str) -> PluginResult<Vec<u8>> {
    let size = std::fs::metadata(path).map_err(|e| io(e, path))?.len();
    if size > MAX_FILE {
        return Err(PluginError::new("info.too_large").with("limit", "200 MB"));
    }
    std::fs::read(path).map_err(|e| io(e, path))
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// 字段的显示文本；去掉 ASCII 值两侧的引号
fn display(exif: &Exif, field: &exif::Field) -> String {
    let text = field.display_value().with_unit(exif).to_string();
    text.trim().trim_matches('"').trim().to_owned()
}

fn field_text(exif: &Exif, tag: Tag) -> Option<String> {
    let text = display(exif, exif.get_field(tag, In::PRIMARY)?);
    (!text.is_empty()).then_some(text)
}

fn rationals(exif: &Exif, tag: Tag) -> Option<Vec<f64>> {
    match &exif.get_field(tag, In::PRIMARY)?.value {
        Value::Rational(values) => Some(values.iter().map(|r| r.to_f64()).collect()),
        _ => None,
    }
}

/// 度分秒 → 十进制度
pub fn dms(values: &[f64], reference: Option<&str>) -> Option<f64> {
    let degrees = values.first()?
        + values.get(1).unwrap_or(&0.0) / 60.0
        + values.get(2).unwrap_or(&0.0) / 3600.0;
    let negative = matches!(reference, Some(r) if r.starts_with('S') || r.starts_with('W'));
    Some(if negative { -degrees } else { degrees })
}

fn gps(exif: &Exif) -> Option<Gps> {
    let lat_ref = field_text(exif, Tag::GPSLatitudeRef);
    let lon_ref = field_text(exif, Tag::GPSLongitudeRef);
    let latitude = dms(&rationals(exif, Tag::GPSLatitude)?, lat_ref.as_deref())?;
    let longitude = dms(&rationals(exif, Tag::GPSLongitude)?, lon_ref.as_deref())?;
    let altitude = rationals(exif, Tag::GPSAltitude)
        .and_then(|v| v.first().copied())
        .map(|a| {
            let below = exif
                .get_field(Tag::GPSAltitudeRef, In::PRIMARY)
                .and_then(|f| f.value.get_uint(0))
                == Some(1);
            if below { -a } else { a }
        });
    Some(Gps {
        latitude: (latitude * 1e6).round() / 1e6,
        longitude: (longitude * 1e6).round() / 1e6,
        altitude: altitude.map(|a| (a * 10.0).round() / 10.0),
    })
}

fn summarize(exif: &Exif) -> Summary {
    Summary {
        make: field_text(exif, Tag::Make),
        model: field_text(exif, Tag::Model),
        lens: field_text(exif, Tag::LensModel),
        taken_at: field_text(exif, Tag::DateTimeOriginal)
            .or_else(|| field_text(exif, Tag::DateTime)),
        exposure: field_text(exif, Tag::ExposureTime),
        aperture: field_text(exif, Tag::FNumber),
        iso: field_text(exif, Tag::PhotographicSensitivity),
        focal_length: field_text(exif, Tag::FocalLength),
        flash: field_text(exif, Tag::Flash),
        software: field_text(exif, Tag::Software),
        orientation: exif
            .get_field(Tag::Orientation, In::PRIMARY)
            .and_then(|f| f.value.get_uint(0)),
        gps: gps(exif),
    }
}

fn read_exif(data: &[u8]) -> Option<Exif> {
    exif::Reader::new()
        .continue_on_error(true)
        .read_from_container(&mut Cursor::new(data))
        .or_else(|e| e.distill_partial_result(|_| {}))
        .ok()
}

fn preview(image: &DynamicImage) -> Option<String> {
    let small = if image.width().max(image.height()) > PREVIEW {
        image.resize(PREVIEW, PREVIEW, FilterType::Triangle)
    } else {
        image.clone()
    };
    let mut buffer = Vec::new();
    DynamicImage::ImageRgba8(small.to_rgba8())
        .write_to(&mut Cursor::new(&mut buffer), ImageFormat::Png)
        .ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        data_encoding::BASE64.encode(&buffer)
    ))
}

pub fn inspect(args: Args) -> PluginResult<Report> {
    let data = read(&args.path)?;
    let reader = ImageReader::new(Cursor::new(&data))
        .with_guessed_format()
        .map_err(|e| PluginError::new("info.decode_failed").with("detail", e.to_string()))?;
    let format = reader.format();
    if format.is_none() {
        return Err(PluginError::new("info.unsupported"));
    }
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| PluginError::new("info.decode_failed").with("detail", e.to_string()))?;
    let orientation = decoder.orientation().ok();
    let color = decoder.color_type();
    let mut image = DynamicImage::from_decoder(decoder)
        .map_err(|e| PluginError::new("info.decode_failed").with("detail", e.to_string()))?;
    let (width, height) = (image.width(), image.height());
    if let Some(orientation) = orientation {
        image.apply_orientation(orientation);
    }
    let divisor = gcd(width, height).max(1);
    let (palette, average) = extract(&image, 6);
    let exif = read_exif(&data);

    Ok(Report {
        name: Path::new(&args.path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        path: args.path,
        size: data.len() as u64,
        format: format.map(|f| format!("{f:?}").to_uppercase()),
        width,
        height,
        aspect: [width / divisor, height / divisor],
        color_type: format!("{color:?}"),
        has_alpha: color.has_alpha(),
        bits_per_pixel: color.bits_per_pixel(),
        preview: preview(&image),
        palette,
        average,
        summary: exif.as_ref().map(summarize),
        exif: exif
            .as_ref()
            .map(|e| {
                e.fields()
                    .map(|f| Entry {
                        ifd: format!("{}", f.ifd_num),
                        tag: f.tag.to_string(),
                        value: display(e, f),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        can_strip: matches!(format, Some(ImageFormat::Jpeg | ImageFormat::Png)),
    })
}

/// 在源文件旁生成不覆盖已有文件的输出路径：`名称-clean.扩展名`
pub fn clean_path(source: &Path) -> PathBuf {
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image".to_owned());
    let ext = source
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dir = source.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut candidate = dir.join(format!("{stem}-clean.{ext}"));
    let mut index = 1;
    while candidate.exists() {
        candidate = dir.join(format!("{stem}-clean ({index}).{ext}"));
        index += 1;
    }
    candidate
}

pub fn strip_metadata(args: StripArgs) -> PluginResult<StripReport> {
    let data = read(&args.path)?;
    let orientation = read_exif(&data).and_then(|e| {
        e.get_field(Tag::Orientation, In::PRIMARY)
            .and_then(|f| f.value.get_uint(0))
    });
    let result = strip(&data)?;
    let output = match args.output.as_deref().filter(|o| !o.is_empty()) {
        Some(path) => PathBuf::from(path),
        None => clean_path(Path::new(&args.path)),
    };
    std::fs::write(&output, &result.bytes).map_err(|e| io(e, &output.to_string_lossy()))?;
    Ok(StripReport {
        output: output.to_string_lossy().into_owned(),
        saved_bytes: (data.len() - result.bytes.len()) as u64,
        removed: result.removed,
        orientation_lost: orientation.is_some_and(|o| o != 1),
    })
}

#[cfg(test)]
#[path = "inspect_test.rs"]
mod tests;
