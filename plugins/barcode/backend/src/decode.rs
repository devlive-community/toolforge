//! 识别图片中的条码与二维码（rxing，多码同时识别）。

use std::io::Cursor;

use image::{GrayImage, ImageReader};
use rxing::common::HybridBinarizer;
use rxing::multi::{GenericMultipleBarcodeReader, MultipleBarcodeReader};
use rxing::{
    BarcodeFormat, BinaryBitmap, DecodeHints, Luma8LuminanceSource, MultiUseMultiFormatReader,
};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

const MAX_FILE: u64 = 50 * 1024 * 1024;

#[derive(Deserialize)]
pub struct DecodeArgs {
    #[serde(default)]
    path: Option<String>,
    /// 识别剪贴板中的图片（例如截图）
    #[serde(default)]
    clipboard: bool,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Decoded {
    /// 本插件的码制 id；不支持生成的码制（如 QR、UPC-E）用 rxing 的名称
    pub format: String,
    pub text: String,
}

fn format_id(format: &BarcodeFormat) -> String {
    match format {
        BarcodeFormat::EAN_13 => "ean13",
        BarcodeFormat::EAN_8 => "ean8",
        BarcodeFormat::UPC_A => "upcA",
        BarcodeFormat::UPC_E => "upcE",
        BarcodeFormat::CODE_128 => "code128",
        BarcodeFormat::CODE_39 => "code39",
        BarcodeFormat::CODE_93 => "code93",
        BarcodeFormat::ITF => "itf",
        BarcodeFormat::CODABAR => "codabar",
        BarcodeFormat::DATA_MATRIX => "dataMatrix",
        BarcodeFormat::PDF_417 => "pdf417",
        BarcodeFormat::AZTEC => "aztec",
        BarcodeFormat::QR_CODE => "qrCode",
        other => return format!("{other:?}"),
    }
    .to_owned()
}

/// 按 ISO-8859-1 读出来的 UTF-8 字节（常见于未声明编码的 Data Matrix）还原成文字
fn utf8_from_latin1(text: &str) -> String {
    if text.is_ascii() || text.chars().any(|c| c as u32 > 0xff) {
        return text.to_owned();
    }
    let bytes: Vec<u8> = text.chars().map(|c| c as u8).collect();
    String::from_utf8(bytes).unwrap_or_else(|_| text.to_owned())
}

pub fn decode_luma(image: &GrayImage) -> PluginResult<Vec<Decoded>> {
    let source =
        Luma8LuminanceSource::new(image.as_raw().clone(), image.width(), image.height())
            .map_err(|e| PluginError::new("barcode.decode_failed").with("detail", e.to_string()))?;
    let mut bitmap = BinaryBitmap::new(HybridBinarizer::new(source));
    let mut reader = GenericMultipleBarcodeReader::new(MultiUseMultiFormatReader::default());
    let hints = DecodeHints {
        TryHarder: Some(true),
        ..Default::default()
    };
    let mut found: Vec<Decoded> = reader
        .decode_multiple_with_hints(&mut bitmap, &hints)
        .unwrap_or_default()
        .iter()
        .map(|r| Decoded {
            format: format_id(r.getBarcodeFormat()),
            text: utf8_from_latin1(r.getText()),
        })
        .collect();
    found.dedup();
    if found.is_empty() {
        return Err(PluginError::new("barcode.not_found"));
    }
    Ok(found)
}

pub fn decode_bytes(bytes: &[u8]) -> PluginResult<Vec<Decoded>> {
    let failed = |e: image::ImageError| {
        PluginError::new("barcode.decode_failed").with("detail", e.to_string())
    };
    let image = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| PluginError::new("barcode.decode_failed").with("detail", e.to_string()))?
        .decode()
        .map_err(failed)?;
    // 透明背景按白底合成，否则透明处会被当成黑色
    let rgba = image.to_rgba8();
    decode_luma(&flatten(&rgba))
}

fn flatten(rgba: &image::RgbaImage) -> GrayImage {
    GrayImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        let luma = (r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000;
        image::Luma([((luma * a as u32 + 255 * (255 - a as u32)) / 255) as u8])
    })
}

fn clipboard_image() -> PluginResult<GrayImage> {
    let empty = || PluginError::new("barcode.clipboard_empty");
    let data = arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.get_image())
        .map_err(|_| empty())?;
    let rgba = image::RgbaImage::from_raw(
        data.width as u32,
        data.height as u32,
        data.bytes.into_owned(),
    )
    .ok_or_else(empty)?;
    Ok(flatten(&rgba))
}

pub fn decode(args: DecodeArgs) -> PluginResult<Vec<Decoded>> {
    if args.clipboard {
        return decode_luma(&clipboard_image()?);
    }
    let path = args.path.unwrap_or_default();
    let metadata = std::fs::metadata(&path)
        .map_err(|_| PluginError::new("fs.not_found").with("path", path.as_str()))?;
    if metadata.len() > MAX_FILE {
        return Err(PluginError::new("barcode.file_too_large").with("limit", "50 MB"));
    }
    let bytes = std::fs::read(&path).map_err(|e| {
        PluginError::new("fs.io")
            .with("path", path.as_str())
            .with("detail", e.to_string())
    })?;
    decode_bytes(&bytes)
}

#[cfg(test)]
#[path = "decode_test.rs"]
mod tests;
