//! 各平台的图标集：文件名、尺寸以及 ICO / ICNS / Contents.json / webmanifest。

use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::{ExtendedColorType, ImageFormat, RgbaImage};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tf_plugin_api::{PluginError, PluginResult};

use crate::compose::{self, MASTER, Shape, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Preset {
    Web,
    Ios,
    Android,
    Macos,
    Windows,
}

/// 按外形缓存母版，避免重复合成
pub struct Masters<'a> {
    source: &'a RgbaImage,
    style: Style,
    cache: BTreeMap<u8, RgbaImage>,
}

impl<'a> Masters<'a> {
    pub fn new(source: &'a RgbaImage, style: Style) -> Self {
        Self {
            source,
            style,
            cache: BTreeMap::new(),
        }
    }

    pub fn get(&mut self, shape: Shape) -> &RgbaImage {
        let key = shape as u8;
        self.cache
            .entry(key)
            .or_insert_with(|| compose::master(self.source, &self.style, shape, MASTER))
    }

    pub fn sized(&mut self, shape: Shape, size: u32) -> RgbaImage {
        compose::scaled(self.get(shape), size)
    }
}

fn write_failed(path: &Path, e: impl ToString) -> PluginError {
    PluginError::new("icon.write_failed")
        .with("path", path.to_string_lossy().as_ref())
        .with("detail", e.to_string())
}

pub fn png_bytes(image: &RgbaImage) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    image
        .write_to(&mut out, ImageFormat::Png)
        .expect("encode png in memory");
    out.into_inner()
}

/// 写出的文件，交给调用方记录日志
pub struct Writer<'a> {
    pub written: Vec<PathBuf>,
    pub on_file: &'a mut dyn FnMut(&Path),
}

impl Writer<'_> {
    fn bytes(&mut self, path: PathBuf, bytes: &[u8]) -> PluginResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| write_failed(parent, e))?;
        }
        std::fs::write(&path, bytes).map_err(|e| write_failed(&path, e))?;
        (self.on_file)(&path);
        self.written.push(path);
        Ok(())
    }

    fn png(&mut self, path: PathBuf, image: &RgbaImage) -> PluginResult<()> {
        self.bytes(path, &png_bytes(image))
    }
}

/// 多尺寸 ICO，每个尺寸以 PNG 存储
pub fn ico(images: &[RgbaImage]) -> PluginResult<Vec<u8>> {
    let pngs: Vec<Vec<u8>> = images.iter().map(png_bytes).collect();
    let frames = images
        .iter()
        .zip(&pngs)
        .map(|(image, png)| {
            IcoFrame::with_encoded(png, image.width(), image.height(), ExtendedColorType::Rgba8)
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| PluginError::new("icon.encode_failed").with("detail", e.to_string()))?;
    let mut out = Vec::new();
    IcoEncoder::new(&mut out)
        .encode_images(&frames)
        .map_err(|e| PluginError::new("icon.encode_failed").with("detail", e.to_string()))?;
    Ok(out)
}

/// ICNS 中的条目：类型与像素尺寸，内容为 PNG
pub const ICNS_ENTRIES: &[(&[u8; 4], u32)] = &[
    (b"icp4", 16),
    (b"icp5", 32),
    (b"ic11", 32),
    (b"ic12", 64),
    (b"ic07", 128),
    (b"ic13", 256),
    (b"ic08", 256),
    (b"ic14", 512),
    (b"ic09", 512),
    (b"ic10", 1024),
];

pub fn icns(png_of: &mut dyn FnMut(u32) -> Vec<u8>) -> Vec<u8> {
    let mut body = Vec::new();
    let mut cache: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    for (kind, size) in ICNS_ENTRIES {
        let png = cache.entry(*size).or_insert_with(|| png_of(*size));
        body.extend_from_slice(*kind);
        body.extend_from_slice(&(png.len() as u32 + 8).to_be_bytes());
        body.extend_from_slice(png);
    }
    let mut out = Vec::with_capacity(body.len() + 8);
    out.extend_from_slice(b"icns");
    out.extend_from_slice(&(body.len() as u32 + 8).to_be_bytes());
    out.extend_from_slice(&body);
    out
}

/// iOS 图标：idiom、点尺寸、倍数
const IOS: &[(&str, &str, u32)] = &[
    ("iphone", "20", 2),
    ("iphone", "20", 3),
    ("iphone", "29", 2),
    ("iphone", "29", 3),
    ("iphone", "40", 2),
    ("iphone", "40", 3),
    ("iphone", "60", 2),
    ("iphone", "60", 3),
    ("ipad", "20", 1),
    ("ipad", "20", 2),
    ("ipad", "29", 1),
    ("ipad", "29", 2),
    ("ipad", "40", 1),
    ("ipad", "40", 2),
    ("ipad", "76", 1),
    ("ipad", "76", 2),
    ("ipad", "83.5", 2),
    ("ios-marketing", "1024", 1),
];

pub fn ios_pixels(points: &str, scale: u32) -> u32 {
    (points.parse::<f32>().unwrap_or(0.0) * scale as f32).round() as u32
}

const ANDROID: &[(&str, u32)] = &[
    ("mdpi", 48),
    ("hdpi", 72),
    ("xhdpi", 96),
    ("xxhdpi", 144),
    ("xxxhdpi", 192),
];

pub const WINDOWS_SIZES: &[u32] = &[16, 24, 32, 48, 64, 128, 256];
pub const FAVICON_SIZES: &[u32] = &[16, 32, 48];

pub const WEB_SNIPPET: &str = r#"<link rel="icon" href="/favicon.ico" sizes="48x48">
<link rel="icon" type="image/png" sizes="32x32" href="/favicon-32x32.png">
<link rel="icon" type="image/png" sizes="16x16" href="/favicon-16x16.png">
<link rel="apple-touch-icon" href="/apple-touch-icon.png">
<link rel="manifest" href="/site.webmanifest">"#;

pub fn write(
    preset: Preset,
    dir: &Path,
    name: &str,
    masters: &mut Masters,
    out: &mut Writer,
) -> PluginResult<()> {
    match preset {
        Preset::Web => {
            let dir = dir.join("web");
            let favicons: Vec<RgbaImage> = FAVICON_SIZES
                .iter()
                .map(|&s| masters.sized(Shape::Rounded, s))
                .collect();
            out.bytes(dir.join("favicon.ico"), &ico(&favicons)?)?;
            out.png(dir.join("favicon-16x16.png"), &favicons[0])?;
            out.png(dir.join("favicon-32x32.png"), &favicons[1])?;
            // iOS 主屏幕图标不支持透明
            out.png(
                dir.join("apple-touch-icon.png"),
                &masters.sized(Shape::Opaque, 180),
            )?;
            for size in [192, 512] {
                out.png(
                    dir.join(format!("android-chrome-{size}x{size}.png")),
                    &masters.sized(Shape::Rounded, size),
                )?;
            }
            let icons: Vec<_> = [192, 512]
                .map(|s| {
                    json!({
                        "src": format!("/android-chrome-{s}x{s}.png"),
                        "sizes": format!("{s}x{s}"),
                        "type": "image/png",
                    })
                })
                .into();
            let manifest = json!({
                "name": name,
                "short_name": name,
                "icons": icons,
                "display": "standalone",
            });
            out.bytes(
                dir.join("site.webmanifest"),
                serde_json::to_string_pretty(&manifest).unwrap().as_bytes(),
            )?;
            out.bytes(
                dir.join("snippet.html"),
                format!("{WEB_SNIPPET}\n").as_bytes(),
            )?;
        }
        Preset::Ios => {
            let dir = dir.join("ios").join("AppIcon.appiconset");
            let mut images = Vec::new();
            let mut done = BTreeMap::new();
            for (idiom, points, scale) in IOS {
                let px = ios_pixels(points, *scale);
                let file = format!("Icon-{px}.png");
                if done.insert(px, ()).is_none() {
                    out.png(dir.join(&file), &masters.sized(Shape::Opaque, px))?;
                }
                images.push(json!({
                    "idiom": idiom,
                    "size": format!("{points}x{points}"),
                    "scale": format!("{scale}x"),
                    "filename": file,
                }));
            }
            let contents = json!({ "images": images, "info": { "author": "xcode", "version": 1 } });
            out.bytes(
                dir.join("Contents.json"),
                serde_json::to_string_pretty(&contents).unwrap().as_bytes(),
            )?;
        }
        Preset::Android => {
            let dir = dir.join("android");
            for (density, size) in ANDROID {
                let folder = dir.join(format!("mipmap-{density}"));
                out.png(
                    folder.join("ic_launcher.png"),
                    &masters.sized(Shape::Rounded, *size),
                )?;
                out.png(
                    folder.join("ic_launcher_round.png"),
                    &masters.sized(Shape::Circle, *size),
                )?;
            }
            out.png(
                dir.join("play-store-512.png"),
                &masters.sized(Shape::Opaque, 512),
            )?;
        }
        Preset::Macos => {
            let mac = masters.get(Shape::Mac).clone();
            let bytes = icns(&mut |size| png_bytes(&compose::scaled(&mac, size)));
            out.bytes(dir.join("macos").join("AppIcon.icns"), &bytes)?;
            out.png(dir.join("macos").join("AppIcon-1024.png"), &mac)?;
        }
        Preset::Windows => {
            let images: Vec<RgbaImage> = WINDOWS_SIZES
                .iter()
                .map(|&s| masters.sized(Shape::Rounded, s))
                .collect();
            out.bytes(dir.join("windows").join("app.ico"), &ico(&images)?)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "presets_test.rs"]
mod tests;
