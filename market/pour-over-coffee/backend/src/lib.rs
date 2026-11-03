//! 手冲咖啡插件后端：粉水比换算、分段注水计划与配方缩放，全部纯计算。
//! 「本段加水量」与「秤上累计值」分别返回，避免现场混淆。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct PourOverCoffee {
    manifest: Manifest,
}

impl Default for PourOverCoffee {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for PourOverCoffee {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "plan" => to_value(plan(parse_args(args)?)?),
            "scale" => to_value(scale(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(PourOverCoffee);

fn positive(value: f64, name: &str) -> PluginResult<f64> {
    if !value.is_finite() || value <= 0.0 {
        return Err(PluginError::new("coffee.positive_required").with("name", name));
    }
    Ok(value)
}

/// 注水计划：coffee_g × ratio = 总注水；stages 给占比或毫升，允许留白（后续自由注水）
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanArgs {
    /// 咖啡粉克数
    pub coffee_g: f64,
    /// 粉水比（如 15 表示 1:15）
    pub ratio: f64,
    /// 分段：占总量比例（0-1）或毫升数，二选一；两者都缺的段按剩余量填
    #[serde(default)]
    pub stages: Vec<StageInput>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageInput {
    #[serde(default)]
    pub fraction: Option<f64>,
    #[serde(default)]
    pub ml: Option<f64>,
    /// 段名（如 bloom / pour2），原样返回
    #[serde(default)]
    pub label: Option<String>,
    /// 该段持续时间（秒，可选）
    #[serde(default)]
    pub seconds: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage {
    pub label: String,
    /// 本段目标注水（毫升）
    pub target_ml: f64,
    /// 秤上累计目标（毫升）
    pub cumulative_ml: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seconds: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanOutput {
    pub coffee_g: f64,
    pub ratio: f64,
    pub total_ml: f64,
    /// 分段合计占总量的比例（1.0 表示分段覆盖全部注水）
    pub covered: f64,
    pub stages: Vec<Stage>,
}

pub fn plan(args: PlanArgs) -> PluginResult<PlanOutput> {
    let coffee = positive(args.coffee_g, "coffeeG")?;
    let ratio = positive(args.ratio, "ratio")?;
    let total = coffee * ratio;
    let mut stages = Vec::new();
    let mut used = 0.0f64;
    for (index, input) in args.stages.iter().enumerate() {
        let target = match (input.fraction, input.ml) {
            (Some(_), Some(_)) => {
                return Err(PluginError::new("coffee.both_fraction_ml").with("index", index as u64));
            }
            (Some(fraction), None) => {
                if !(0.0..=1.0).contains(&fraction) {
                    return Err(PluginError::new("coffee.bad_fraction").with("index", index as u64));
                }
                total * fraction
            }
            (_, Some(ml)) => positive(ml, "ml")?,
            (None, None) => {
                return Err(PluginError::new("coffee.stage_empty").with("index", index as u64));
            }
        };
        used += target;
        if used > total + 1e-6 {
            return Err(PluginError::new("coffee.stage_overflow").with("index", index as u64));
        }
        stages.push(Stage {
            label: input
                .label
                .clone()
                .unwrap_or_else(|| format!("{}", index + 1)),
            target_ml: round2(target),
            cumulative_ml: round2(used),
            seconds: input.seconds,
        });
    }
    Ok(PlanOutput {
        coffee_g: coffee,
        ratio,
        total_ml: round2(total),
        covered: round4(used / total),
        stages,
    })
}

/// 配方缩放：把原配方的分段按新粉量等比放大
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScaleArgs {
    /// 原配方粉量
    pub original_coffee_g: f64,
    /// 新粉量
    pub new_coffee_g: f64,
    /// 原配方分段毫升数
    pub stage_ml: Vec<f64>,
    /// 原粉水比（用于校验分段总和）
    #[serde(default)]
    pub original_ratio: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScaleOutput {
    pub new_coffee_g: f64,
    pub factor: f64,
    pub new_total_ml: f64,
    pub stage_ml: Vec<f64>,
}

pub fn scale(args: ScaleArgs) -> PluginResult<ScaleOutput> {
    let original = positive(args.original_coffee_g, "originalCoffeeG")?;
    let new = positive(args.new_coffee_g, "newCoffeeG")?;
    if args.stage_ml.is_empty() {
        return Err(PluginError::new("coffee.stage_empty").with("index", 0));
    }
    let factor = new / original;
    let scaled: Vec<f64> = args.stage_ml.iter().map(|ml| round2(ml * factor)).collect();
    if let Some(ratio) = args.original_ratio {
        let total = original * positive(ratio, "originalRatio")?;
        let sum: f64 = args.stage_ml.iter().sum();
        if sum > total + 0.5 {
            return Err(
                PluginError::new("coffee.stage_overflow").with("index", args.stage_ml.len() as u64)
            );
        }
    }
    Ok(ScaleOutput {
        new_coffee_g: new,
        factor: round4(factor),
        new_total_ml: round2(new * args.original_ratio.unwrap_or(0.0)),
        stage_ml: scaled,
    })
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
