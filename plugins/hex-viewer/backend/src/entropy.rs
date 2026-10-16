//! 熵分析：按块计算香农熵（0–8 比特 / 字节），找出高熵区段（压缩或加密数据）。

use std::io::Read;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext, cancelled};

use crate::file;

/// 曲线最多的点数；文件太大时自动放大块大小
pub const MAX_POINTS: u64 = 4096;
/// 不低于这个值视为高熵（压缩 / 加密数据通常接近 8）
pub const HIGH: f32 = 7.2;
const MIN_BLOCK: u64 = 64;
const MAX_BLOCK: u64 = 1 << 30;
const MAX_REGIONS: usize = 200;

#[derive(Deserialize)]
pub struct Args {
    pub path: String,
    /// 期望的块大小（字节）
    pub block: u64,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Region {
    pub start: u64,
    pub end: u64,
    pub average: f32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// 实际使用的块大小
    pub block: u64,
    pub values: Vec<f32>,
    pub overall: f32,
    pub regions: Vec<Region>,
    pub regions_truncated: bool,
    pub threshold: f32,
}

pub fn shannon(counts: &[u64; 256], total: u64) -> f32 {
    if total == 0 {
        return 0.0;
    }
    let total = total as f64;
    let bits: f64 = counts
        .iter()
        .filter(|c| **c > 0)
        .map(|c| {
            let p = *c as f64 / total;
            -p * p.log2()
        })
        .sum();
    ((bits * 1000.0).round() / 1000.0) as f32
}

/// 块大小向上取到 2 的幂，并保证点数不超过 MAX_POINTS
pub fn block_size(requested: u64, size: u64) -> u64 {
    let mut block = requested.clamp(MIN_BLOCK, MAX_BLOCK).next_power_of_two();
    while size.div_ceil(block) > MAX_POINTS && block < MAX_BLOCK {
        block *= 2;
    }
    block
}

/// 把连续的高熵块合并为区段
pub fn regions(values: &[f32], block: u64, size: u64) -> (Vec<Region>, bool) {
    let mut out = Vec::new();
    let mut current: Option<(usize, f64)> = None;
    let close = |out: &mut Vec<Region>, start: usize, end: usize, sum: f64| {
        out.push(Region {
            start: start as u64 * block,
            end: (end as u64 * block).min(size),
            average: ((sum / (end - start) as f64) * 1000.0).round() as f32 / 1000.0,
        });
    };
    for (index, value) in values.iter().enumerate() {
        match (&mut current, *value >= HIGH) {
            (Some((_, sum)), true) => *sum += f64::from(*value),
            (None, true) => current = Some((index, f64::from(*value))),
            (Some((start, sum)), false) => {
                close(&mut out, *start, index, *sum);
                current = None;
            }
            (None, false) => {}
        }
    }
    if let Some((start, sum)) = current {
        close(&mut out, start, values.len(), sum);
    }
    let truncated = out.len() > MAX_REGIONS;
    out.truncate(MAX_REGIONS);
    (out, truncated)
}

pub fn run(args: Args, ctx: &dyn TaskContext) -> PluginResult<Report> {
    let (mut handle, size) = file::open(&args.path)?;
    if size == 0 {
        return Err(PluginError::new("hex.empty_file"));
    }
    let block = block_size(args.block, size);
    ctx.log(
        LogLevel::Info,
        "hex.entropy_start",
        json!({ "block": block, "points": size.div_ceil(block) }),
    );
    let mut values = Vec::with_capacity(size.div_ceil(block) as usize);
    let mut total = [0u64; 256];
    let mut buffer = vec![0u8; block.min(file::MAX_READ) as usize];
    let mut done = 0u64;
    while done < size {
        let mut counts = [0u64; 256];
        let mut in_block = 0u64;
        while in_block < block && done < size {
            if ctx.is_cancelled() {
                return Err(cancelled());
            }
            let want = (block - in_block).min(buffer.len() as u64) as usize;
            let read = handle.read(&mut buffer[..want]).map_err(|e| {
                PluginError::new("hex.read_failed")
                    .with("path", args.path.as_str())
                    .with("detail", e.to_string())
            })?;
            if read == 0 {
                done = size;
                break;
            }
            for byte in &buffer[..read] {
                counts[*byte as usize] += 1;
            }
            in_block += read as u64;
            done += read as u64;
            ctx.progress(done, size);
        }
        for (sum, count) in total.iter_mut().zip(counts) {
            *sum += count;
        }
        if in_block > 0 {
            values.push(shannon(&counts, in_block));
        }
    }
    let (regions, regions_truncated) = regions(&values, block, size);
    let overall = shannon(&total, total.iter().sum());
    ctx.log(
        LogLevel::Info,
        "hex.entropy_done",
        json!({ "overall": overall, "regions": regions.len() }),
    );
    Ok(Report {
        block,
        values,
        overall,
        regions,
        regions_truncated,
        threshold: HIGH,
    })
}

#[cfg(test)]
#[path = "entropy_test.rs"]
mod tests;
