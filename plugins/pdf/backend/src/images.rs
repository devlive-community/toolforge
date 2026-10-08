//! 图片转 PDF：JPEG 原样嵌入（不重新压缩），其他格式转为无损压缩的 RGB，透明度写入软蒙版。

use std::io::Cursor;
use std::path::Path;

use image::{DynamicImage, ImageFormat, ImageReader};
use lopdf::{Dictionary, Document, Object, Stream};
use serde::Deserialize;
use tf_plugin_api::{PluginError, PluginResult};

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PageSize {
    /// 页面与图片同样大小（按 72 DPI）
    #[default]
    Fit,
    A4,
    Letter,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub paths: Vec<String>,
    pub output: String,
    #[serde(default)]
    pub page_size: PageSize,
    /// 页边距（点），A4 / Letter 时生效
    #[serde(default)]
    pub margin: f32,
}

fn decode_failed(path: &Path, err: impl ToString) -> PluginError {
    PluginError::new("pdf.image_unsupported")
        .with("path", path.to_string_lossy().as_ref())
        .with("detail", err.to_string())
}

/// 图片 XObject 与其像素尺寸
fn image_object(doc: &mut Document, path: &Path) -> PluginResult<(Object, u32, u32)> {
    let bytes = std::fs::read(path).map_err(|e| decode_failed(path, e))?;
    let reader = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|e| decode_failed(path, e))?;
    let format = reader.format();
    let image = reader.decode().map_err(|e| decode_failed(path, e))?;
    let (width, height) = (image.width(), image.height());
    let mut dict = Dictionary::new();
    dict.set("Type", Object::Name(b"XObject".to_vec()));
    dict.set("Subtype", Object::Name(b"Image".to_vec()));
    dict.set("Width", width as i64);
    dict.set("Height", height as i64);
    dict.set("BitsPerComponent", 8);
    // RGB / 灰度 JPEG 直接嵌入原始数据，避免二次压缩损失
    let passthrough = format == Some(ImageFormat::Jpeg)
        && matches!(
            image,
            DynamicImage::ImageRgb8(_) | DynamicImage::ImageLuma8(_)
        );
    if passthrough {
        let gray = matches!(image, DynamicImage::ImageLuma8(_));
        dict.set(
            "ColorSpace",
            Object::Name(if gray {
                b"DeviceGray".to_vec()
            } else {
                b"DeviceRGB".to_vec()
            }),
        );
        dict.set("Filter", Object::Name(b"DCTDecode".to_vec()));
        return Ok((Object::Stream(Stream::new(dict, bytes)), width, height));
    }
    dict.set("ColorSpace", Object::Name(b"DeviceRGB".to_vec()));
    if image.color().has_alpha() {
        let rgba = image.to_rgba8();
        let alpha: Vec<u8> = rgba.pixels().map(|p| p[3]).collect();
        let mut mask_dict = Dictionary::new();
        mask_dict.set("Type", Object::Name(b"XObject".to_vec()));
        mask_dict.set("Subtype", Object::Name(b"Image".to_vec()));
        mask_dict.set("Width", width as i64);
        mask_dict.set("Height", height as i64);
        mask_dict.set("BitsPerComponent", 8);
        mask_dict.set("ColorSpace", Object::Name(b"DeviceGray".to_vec()));
        let mut mask = Stream::new(mask_dict, alpha);
        let _ = mask.compress();
        let mask_id = doc.add_object(mask);
        dict.set("SMask", mask_id);
    }
    let mut stream = Stream::new(dict, image.to_rgb8().into_raw());
    let _ = stream.compress();
    Ok((Object::Stream(stream), width, height))
}

/// 页面尺寸与图片在页面上的位置（x, y, w, h），单位为点
pub fn layout(
    size: PageSize,
    margin: f32,
    width: u32,
    height: u32,
) -> ((f32, f32), (f32, f32, f32, f32)) {
    let (w, h) = (width as f32 * 0.75, height as f32 * 0.75);
    let (page_w, page_h) = match size {
        PageSize::Fit => return ((w.max(1.0), h.max(1.0)), (0.0, 0.0, w, h)),
        PageSize::A4 => (595.28_f32, 841.89_f32),
        PageSize::Letter => (612.0, 792.0),
    };
    // 横向图片使用横向页面
    let (page_w, page_h) = if width > height {
        (page_h, page_w)
    } else {
        (page_w, page_h)
    };
    let margin = margin.clamp(0.0, page_w.min(page_h) / 3.0);
    let (box_w, box_h) = (page_w - margin * 2.0, page_h - margin * 2.0);
    // 等比缩放放入可用区域，小图不放大
    let scale = (box_w / w).min(box_h / h).min(1.0);
    let (draw_w, draw_h) = (w * scale, h * scale);
    (
        (page_w, page_h),
        (
            (page_w - draw_w) / 2.0,
            (page_h - draw_h) / 2.0,
            draw_w,
            draw_h,
        ),
    )
}

pub fn images_to_pdf(
    args: &Args,
    mut progress: impl FnMut(usize),
    is_cancelled: impl Fn() -> bool,
) -> PluginResult<Document> {
    if args.paths.is_empty() {
        return Err(PluginError::new("pdf.no_images"));
    }
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let mut kids = Vec::new();
    for (index, raw) in args.paths.iter().enumerate() {
        if is_cancelled() {
            return Err(tf_plugin_api::cancelled());
        }
        let (image, width, height) = image_object(&mut doc, Path::new(raw))?;
        let image_id = doc.add_object(image);
        let ((page_w, page_h), (x, y, w, h)) = layout(args.page_size, args.margin, width, height);
        let content = format!("q {w:.3} 0 0 {h:.3} {x:.3} {y:.3} cm /Im0 Do Q");
        let content_id = doc.add_object(Stream::new(Dictionary::new(), content.into_bytes()));
        let mut xobjects = Dictionary::new();
        xobjects.set("Im0", image_id);
        let mut resources = Dictionary::new();
        resources.set("XObject", xobjects);
        let mut page = Dictionary::new();
        page.set("Type", Object::Name(b"Page".to_vec()));
        page.set("Parent", pages_id);
        page.set(
            "MediaBox",
            vec![
                0.into(),
                0.into(),
                Object::Real(page_w),
                Object::Real(page_h),
            ],
        );
        page.set("Resources", resources);
        page.set("Contents", content_id);
        kids.push(Object::Reference(doc.add_object(page)));
        progress(index + 1);
    }
    let count = kids.len() as i64;
    let mut pages = Dictionary::new();
    pages.set("Type", Object::Name(b"Pages".to_vec()));
    pages.set("Kids", Object::Array(kids));
    pages.set("Count", count);
    doc.objects.insert(pages_id, Object::Dictionary(pages));
    let mut catalog = Dictionary::new();
    catalog.set("Type", Object::Name(b"Catalog".to_vec()));
    catalog.set("Pages", pages_id);
    let catalog_id = doc.add_object(catalog);
    let mut info = Dictionary::new();
    info.set("Producer", lopdf::text_string("ToolForge"));
    let info_id = doc.add_object(info);
    doc.trailer.set("Root", catalog_id);
    doc.trailer.set("Info", info_id);
    Ok(doc)
}

#[cfg(test)]
#[path = "images_test.rs"]
mod tests;
