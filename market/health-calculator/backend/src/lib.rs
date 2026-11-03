//! 健康计算插件后端：BMI、腰臀比、BMR/TDEE、体表面积、理想体重、运动心率与
//! 饮水/蛋白质参考。所有结果仅供参考，界面标注不构成医疗建议。
//!
//! 公式来源（界面上同时展示）：BMI 分档用中国标准（WS/T 428）并附 WHO 对照；
//! BMR 用 Mifflin-St Jeor；体表面积用 Du Bois；理想体重用 Devine（按 152.4cm 基线 45.5/50kg，每超出 2.54cm 加 2.3/2.7kg）。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct HealthCalculator {
    manifest: Manifest,
}

impl Default for HealthCalculator {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for HealthCalculator {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "bmi" => to_value(bmi(parse_args(args)?)?),
            "energy" => to_value(energy(parse_args(args)?)?),
            "heart_rate" => to_value(heart_rate(parse_args(args)?)?),
            "body" => to_value(body(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(HealthCalculator);

fn positive(value: f64, name: &str) -> PluginResult<f64> {
    if value <= 0.0 {
        return Err(PluginError::new("health.positive_required").with("name", name));
    }
    Ok(value)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BmiArgs {
    /// 身高（厘米）
    pub height: f64,
    /// 体重（公斤）
    pub weight: f64,
    /// 腰围（厘米，可选）
    #[serde(default)]
    pub waist: Option<f64>,
    /// 臀围（厘米，可选）
    #[serde(default)]
    pub hip: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmiOutput {
    pub bmi: f64,
    /// 中国标准分档键：underweight/normal/overweight/obese
    pub cn_category: &'static str,
    /// WHO 分档键
    pub who_category: &'static str,
    /// 中国标准的健康体重区间（公斤）
    pub healthy_min: f64,
    pub healthy_max: f64,
    pub whr: Option<f64>,
    /// 腰臀比参考：male/female 下 男性≥0.9、女性≥0.85 为中央型肥胖
    pub whr_risk: Option<bool>,
}

pub fn bmi(args: BmiArgs) -> PluginResult<BmiOutput> {
    let height = positive(args.height, "height")?;
    let weight = positive(args.weight, "weight")?;
    let m = height / 100.0;
    let bmi = weight / (m * m);
    let cn = if bmi < 18.5 {
        "underweight"
    } else if bmi < 24.0 {
        "normal"
    } else if bmi < 28.0 {
        "overweight"
    } else {
        "obese"
    };
    let who = if bmi < 18.5 {
        "underweight"
    } else if bmi < 25.0 {
        "normal"
    } else if bmi < 30.0 {
        "overweight"
    } else {
        "obese"
    };
    let (whr, risk) = match (args.waist, args.hip) {
        (Some(w), Some(h)) if w > 0.0 && h > 0.0 => {
            let ratio = w / h;
            // 性别信息在 body 接口里；这里按男 0.9 阈值给保守参考，
            // 界面通过 body 接口可拿到更精确的分档
            (Some(ratio), Some(ratio >= 0.9))
        }
        _ => (None, None),
    };
    Ok(BmiOutput {
        bmi,
        cn_category: cn,
        who_category: who,
        healthy_min: 18.5 * m * m,
        healthy_max: 23.9 * m * m,
        whr,
        whr_risk: risk,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnergyArgs {
    pub height: f64,
    pub weight: f64,
    /// 岁
    pub age: f64,
    /// male / female
    pub sex: String,
    /// 活动系数：1.2 / 1.375 / 1.55 / 1.725 / 1.9（久坐到高强度）
    #[serde(default)]
    pub activity: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnergyOutput {
    /// Mifflin-St Jeor 基础代谢（千卡/天）
    pub bmr: f64,
    /// 每日总消耗 = BMR × 活动系数
    pub tdee: f64,
    /// 减脂目标（TDEE − 500）
    pub cut: f64,
    /// 增重目标（TDEE + 300）
    pub bulk: f64,
}

pub fn energy(args: EnergyArgs) -> PluginResult<EnergyOutput> {
    let height = positive(args.height, "height")?;
    let weight = positive(args.weight, "weight")?;
    let age = positive(args.age, "age")?;
    if !matches!(args.sex.as_str(), "male" | "female") {
        return Err(PluginError::new("health.bad_sex").with("sex", args.sex));
    }
    let activity = args.activity.unwrap_or(1.55);
    if !(1.0..=2.5).contains(&activity) {
        return Err(PluginError::new("health.bad_activity").with("value", activity));
    }
    let sex_offset = if args.sex == "male" { 5.0 } else { -161.0 };
    let bmr = 10.0 * weight + 6.25 * height - 5.0 * age + sex_offset;
    let tdee = bmr * activity;
    Ok(EnergyOutput {
        bmr,
        tdee,
        cut: tdee - 500.0,
        bulk: tdee + 300.0,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeartRateArgs {
    /// 岁
    pub age: f64,
    /// 静息心率（可选，默认 60）
    #[serde(default)]
    pub resting: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeartRateOutput {
    pub max: f64,
    pub resting: f64,
    /// 五区间 [低, 高]：暖身 50-60%、燃脂 60-70%、有氧 70-80%、无氧 80-90%、极限 90-100%
    pub zones: Vec<Zone>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Zone {
    /// i18n 键：zones.warmup 等
    pub label: &'static str,
    pub low: f64,
    pub high: f64,
}

pub fn heart_rate(args: HeartRateArgs) -> PluginResult<HeartRateOutput> {
    let age = positive(args.age, "age")?;
    if age > 120.0 {
        return Err(PluginError::new("health.bad_age").with("age", age));
    }
    let resting = args.resting.unwrap_or(60.0);
    let resting = positive(resting, "resting")?;
    let max = 220.0 - age;
    let reserve = max - resting;
    let zones = [
        ("warmup", 0.50, 0.60),
        ("fat", 0.60, 0.70),
        ("aerobic", 0.70, 0.80),
        ("anaerobic", 0.80, 0.90),
        ("peak", 0.90, 1.00),
    ]
    .into_iter()
    .map(|(label, low, high)| Zone {
        label,
        low: resting + reserve * low,
        high: resting + reserve * high,
    })
    .collect();
    Ok(HeartRateOutput {
        max,
        resting,
        zones,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyArgs {
    pub height: f64,
    pub weight: f64,
    /// male / female
    pub sex: String,
    #[serde(default)]
    pub waist: Option<f64>,
    #[serde(default)]
    pub hip: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyOutput {
    /// Du Bois 体表面积（平方米）
    pub bsa: f64,
    /// Devine 理想体重（公斤）
    pub ideal_weight: f64,
    /// 理想体重 ±10% 区间
    pub ideal_min: f64,
    pub ideal_max: f64,
    /// 每日饮水参考（升，按 30–35 ml/kg 取中值 33）
    pub water_liters: f64,
    /// 蛋白质参考（克/天，普通成人 1.0–1.2 g/kg 取 1.1）
    pub protein_grams: f64,
    /// 腰臀比（提供腰臀围时）
    pub whr: Option<f64>,
    /// 中央型肥胖参考阈值（男 0.9 / 女 0.85）
    pub whr_risk: Option<bool>,
}

pub fn body(args: BodyArgs) -> PluginResult<BodyOutput> {
    let height = positive(args.height, "height")?;
    let weight = positive(args.weight, "weight")?;
    if !matches!(args.sex.as_str(), "male" | "female") {
        return Err(PluginError::new("health.bad_sex").with("sex", args.sex));
    }
    let m = height / 100.0;
    let bsa = 0.007_184 * m.powf(0.725) * weight.powf(0.425);
    let inches = (height - 152.4) / 2.54;
    let base = if args.sex == "male" { 50.0 } else { 45.5 };
    let per_inch = if args.sex == "male" { 2.3 } else { 2.2 };
    let ideal = base + inches.max(0.0) * per_inch;
    let whr = match (args.waist, args.hip) {
        (Some(w), Some(h)) if w > 0.0 && h > 0.0 => {
            let ratio = w / h;
            Some(ratio)
        }
        _ => None,
    };
    let risk = whr.map(|ratio| {
        if args.sex == "male" {
            ratio >= 0.9
        } else {
            ratio >= 0.85
        }
    });
    Ok(BodyOutput {
        bsa,
        ideal_weight: ideal,
        ideal_min: ideal * 0.9,
        ideal_max: ideal * 1.1,
        water_liters: weight * 0.033,
        protein_grams: weight * 1.1,
        whr,
        whr_risk: risk,
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
