//! 命令行 / Agent 用的扁平入口：用文字列出完成的组，算出组数、总次数、总训练量与最大重量。
//!
//! 每组写法：`次数x重量[kg|lb]`（如 `8x60kg`、`5x135lb`），只写次数表示自重动作；
//! 不写单位时用 unit 参数。次数与重量的校验、磅到千克的换算都复用界面同一套规则。

use serde::{Deserialize, Serialize};
use tf_plugin_api::PluginResult;

use crate::model::{Set, Status, Unit, error, kilograms, repetitions};

/// 每次最多统计的组数（与单次训练上限同量级）
const MAX_SETS: usize = 2000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeArgs {
    pub sets: Vec<String>,
    #[serde(default)]
    pub unit: Unit,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VolumeRow {
    pub sets: u64,
    pub reps: u64,
    /// 负重组的 Σ(次数 × 千克)，保留 3 位小数
    pub volume_kg: String,
    /// 单组最大重量（千克）；全为自重时为空
    pub heaviest_kg: String,
}

/// 汇总放在单行表格里，命令行按语言显示列名
#[derive(Debug, Serialize, PartialEq)]
pub struct VolumeOut {
    pub summary: Vec<VolumeRow>,
}

fn parse(line: &str, default: Unit) -> PluginResult<Set> {
    let compact: String = line
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let (reps, load) = match compact.split_once(['x', '×', '*']) {
        Some((reps, load)) => (reps.to_string(), load.to_string()),
        None => (compact.clone(), String::new()),
    };
    let (load, unit) = if let Some(n) = load.strip_suffix("kg") {
        (n.to_string(), Unit::Kg)
    } else if let Some(n) = load.strip_suffix("lb").or_else(|| load.strip_suffix("lbs")) {
        (n.to_string(), Unit::Lb)
    } else {
        (load, default)
    };
    if repetitions(&reps)?.is_none() {
        return Err(error("reps"));
    }
    Ok(Set {
        reps,
        weight: load,
        unit,
        warmup: false,
        note: String::new(),
        status: Status::Done,
    })
}

pub fn volume(args: VolumeArgs) -> PluginResult<VolumeOut> {
    if args.sets.is_empty() || args.sets.len() > MAX_SETS {
        return Err(error("sets"));
    }
    let mut reps = 0u64;
    let mut volume = 0f64;
    let mut heaviest: Option<f64> = None;
    for line in &args.sets {
        let set = parse(line, args.unit)?;
        let count = repetitions(&set.reps)?.unwrap_or(0);
        reps += u64::from(count);
        if let Some(kg) = kilograms(&set)? {
            volume += kg * f64::from(count);
            heaviest = Some(heaviest.map_or(kg, |h| h.max(kg)));
        }
    }
    Ok(VolumeOut {
        summary: vec![VolumeRow {
            sets: args.sets.len() as u64,
            reps,
            volume_kg: format!("{volume:.3}"),
            heaviest_kg: heaviest.map(|h| format!("{h:.3}")).unwrap_or_default(),
        }],
    })
}

#[cfg(test)]
#[path = "flat_test.rs"]
mod tests;
