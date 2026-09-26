//! 文字识别（CRNN + CTC）的前后处理：文字行缩放到 48 像素高并补齐到固定宽度档位，
//! 输出按贪心 CTC 解码为字符串。

use std::sync::OnceLock;

use image::RgbImage;
use image::imageops::FilterType;
use tract_onnx::prelude::*;

pub const HEIGHT: u32 = 48;
/// 识别输入的宽度档位：tract 需要固定形状，每个档位编译一次并缓存
const BUCKETS: [u32; 8] = [160, 320, 480, 640, 960, 1280, 1920, 2560];
/// 低于该置信度的行视为噪声
pub const MIN_SCORE: f32 = 0.5;

const DICTIONARY: &str = include_str!("../assets/ppocr_keys_v1.txt");

/// 字典：下标 0 为 CTC 空白，末尾追加空格
pub fn characters() -> &'static [&'static str] {
    static KEYS: OnceLock<Vec<&'static str>> = OnceLock::new();
    KEYS.get_or_init(|| {
        std::iter::once("")
            .chain(DICTIONARY.lines())
            .chain(std::iter::once(" "))
            .collect()
    })
}

pub fn bucket(width: u32) -> u32 {
    BUCKETS
        .into_iter()
        .find(|b| *b >= width)
        .unwrap_or(BUCKETS[BUCKETS.len() - 1])
}

/// 返回（输入张量，档位宽度）；超出最大档位的长行会被压缩
pub fn input(line: &RgbImage) -> (Tensor, u32) {
    let (w, h) = line.dimensions();
    let width = ((HEIGHT as f32 * w as f32 / h.max(1) as f32).ceil() as u32).max(1);
    let bucket = bucket(width);
    let width = width.min(bucket);
    let resized = image::imageops::resize(line, width, HEIGHT, FilterType::Triangle);
    // 右侧补 0（归一化后的中间值），与 PaddleOCR 一致
    let tensor = tract_ndarray::Array4::from_shape_fn(
        (1, 3, HEIGHT as usize, bucket as usize),
        |(_, c, y, x)| {
            if x as u32 >= width {
                return 0.0;
            }
            let pixel = resized.get_pixel(x as u32, y as u32).0;
            (pixel[2 - c] as f32 / 255.0 - 0.5) / 0.5
        },
    );
    (tensor.into(), bucket)
}

/// 贪心 CTC 解码：每步取最大概率，去掉空白与连续重复；分数为保留字符的平均概率
pub fn decode(probs: &[f32], steps: usize, classes: usize, keys: &[&str]) -> (String, f32) {
    let mut text = String::new();
    let (mut total, mut kept, mut last) = (0.0f32, 0usize, 0usize);
    for step in 0..steps {
        let row = &probs[step * classes..(step + 1) * classes];
        let (index, best) =
            row.iter().enumerate().fold(
                (0, f32::MIN),
                |acc, (i, &v)| if v > acc.1 { (i, v) } else { acc },
            );
        if index != 0
            && index != last
            && let Some(key) = keys.get(index)
        {
            text.push_str(key);
            total += best;
            kept += 1;
        }
        last = index;
    }
    let score = if kept == 0 { 0.0 } else { total / kept as f32 };
    (text.trim().to_owned(), score)
}

#[cfg(test)]
#[path = "recognize_test.rs"]
mod tests;
