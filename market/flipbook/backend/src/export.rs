//! 导出：循环 GIF 或 PNG 帧序列，帧顺序、尺寸与停留时长和预览一致。
//!
//! - GIF：每帧先合成到用户选择的不透明底色上，整帧覆盖，循环播放不会残留上一帧；
//!   全部编码完成后才一次性写入目标文件，取消或失败都不会留下半个文件。
//! - PNG 序列：保留透明，写入用户选择的文件夹，文件名为 `<名称>-001.png` 起依次编号。
//! - 只读取界面传来的帧内容，不改动作品存档；洋葱皮等辅助显示本来就不在帧数据里。

use image::RgbaImage;
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginResult, TaskContext, cancelled};

use crate::frame::{self, MAX_FRAMES, error, parse_color};
use crate::project::{MAX_DELAY, MIN_DELAY};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Gif,
    Png,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub format: Format,
    pub width: u32,
    pub height: u32,
    pub delay_ms: u32,
    /// GIF 底色 `#rrggbb`
    #[serde(default = "white")]
    pub background: String,
    /// 按播放顺序的帧存档文本
    pub frames: Vec<String>,
    /// GIF：目标文件；PNG：目标文件夹
    pub path: String,
    /// PNG 文件名前缀
    #[serde(default)]
    pub name: String,
}

fn white() -> String {
    "#ffffff".to_string()
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub format: String,
    pub frames: usize,
    /// 写入的文件数
    pub files: usize,
    pub bytes: u64,
}

/// 写文件的方式（宿主写入；测试中替换为内存）
pub trait Sink {
    fn write(&mut self, path: &str, data: &[u8]) -> PluginResult<()>;
}

fn check(ctx: &dyn TaskContext) -> PluginResult<()> {
    if ctx.is_cancelled() {
        Err(cancelled())
    } else {
        Ok(())
    }
}

/// 文件名前缀：去掉路径分隔符与控制字符，留空时用 frame
pub fn file_stem(name: &str) -> String {
    let stem: String = name
        .trim()
        .chars()
        .filter(|c| {
            !c.is_control() && !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
        .take(40)
        .collect();
    let stem = stem
        .trim_matches(|c: char| c == '.' || c.is_whitespace())
        .to_string();
    if stem.is_empty() {
        "frame".to_string()
    } else {
        stem
    }
}

/// 拼接文件夹与文件名，沿用文件夹路径里的分隔符风格
pub fn join(dir: &str, file: &str) -> String {
    let sep = if dir.contains('\\') && !dir.contains('/') {
        '\\'
    } else {
        '/'
    };
    if dir.ends_with(sep) {
        format!("{dir}{file}")
    } else {
        format!("{dir}{sep}{file}")
    }
}

pub fn run(req: &Request, ctx: &dyn TaskContext, sink: &mut dyn Sink) -> PluginResult<Outcome> {
    frame::check_size(req.width, req.height)?;
    if req.frames.is_empty() || req.frames.len() > MAX_FRAMES {
        return Err(error("bad_project"));
    }
    if !(MIN_DELAY..=MAX_DELAY).contains(&req.delay_ms) || !req.delay_ms.is_multiple_of(10) {
        return Err(error("bad_delay"));
    }
    if req.path.is_empty() || req.path.len() > 4096 {
        return Err(error("bad_path"));
    }
    let total = req.frames.len();
    ctx.stage("decode");
    // 先完整解码校验所有帧，任何一帧有问题都不写文件
    let mut images = Vec::with_capacity(total);
    for text in &req.frames {
        check(ctx)?;
        images.push(frame::decode(text, req.width, req.height)?);
    }
    match req.format {
        Format::Gif => gif(req, &images, ctx, sink),
        Format::Png => png(req, &images, ctx, sink),
    }
}

fn gif(
    req: &Request,
    images: &[RgbaImage],
    ctx: &dyn TaskContext,
    sink: &mut dyn Sink,
) -> PluginResult<Outcome> {
    let bg = parse_color(&req.background)?;
    let (w, h) = (req.width as u16, req.height as u16);
    let mut out = Vec::new();
    {
        let mut encoder = gif::Encoder::new(&mut out, w, h, &[]).map_err(|_| error("encode"))?;
        encoder
            .set_repeat(gif::Repeat::Infinite)
            .map_err(|_| error("encode"))?;
        ctx.stage("encode");
        for (i, image) in images.iter().enumerate() {
            check(ctx)?;
            let mut rgb = Vec::with_capacity((req.width * req.height * 3) as usize);
            for p in image.pixels() {
                let a = u16::from(p[3]);
                for c in 0..3 {
                    let v = (u16::from(p[c]) * a + u16::from(bg[c]) * (255 - a) + 127) / 255;
                    rgb.push(v as u8);
                }
            }
            let mut frame = gif::Frame::from_rgb_speed(w, h, &rgb, 10);
            frame.delay = (req.delay_ms / 10) as u16;
            // 整帧不透明覆盖，播放下一帧前不保留上一帧内容
            frame.dispose = gif::DisposalMethod::Background;
            encoder.write_frame(&frame).map_err(|_| error("encode"))?;
            ctx.progress(i as u64 + 1, images.len() as u64);
        }
    }
    check(ctx)?;
    ctx.stage("write");
    sink.write(&req.path, &out)?;
    Ok(Outcome {
        format: "gif".to_string(),
        frames: images.len(),
        files: 1,
        bytes: out.len() as u64,
    })
}

fn png(
    req: &Request,
    images: &[RgbaImage],
    ctx: &dyn TaskContext,
    sink: &mut dyn Sink,
) -> PluginResult<Outcome> {
    let stem = file_stem(&req.name);
    let digits = images.len().to_string().len().max(3);
    // 先全部编码到内存，确认无误再逐个写入
    ctx.stage("encode");
    let mut encoded = Vec::with_capacity(images.len());
    for (i, image) in images.iter().enumerate() {
        check(ctx)?;
        encoded.push(frame::png_bytes(image)?);
        ctx.progress(i as u64 + 1, images.len() as u64 * 2);
    }
    ctx.stage("write");
    let mut bytes = 0u64;
    for (i, data) in encoded.iter().enumerate() {
        check(ctx)?;
        let file = format!("{stem}-{:0digits$}.png", i + 1);
        sink.write(&join(&req.path, &file), data)?;
        bytes += data.len() as u64;
        ctx.progress((images.len() + i + 1) as u64, images.len() as u64 * 2);
    }
    Ok(Outcome {
        format: "png".to_string(),
        frames: images.len(),
        files: images.len(),
        bytes,
    })
}

#[cfg(test)]
#[path = "export_test.rs"]
mod tests;
