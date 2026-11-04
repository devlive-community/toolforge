//! 元素周期表后端：118 元素核心数据（中文名/符号/原子量/电子排布/类别）与
//! 摩尔质量计算。数据为公有领域科学事实。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct PeriodicTable {
    manifest: Manifest,
}

impl Default for PeriodicTable {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for PeriodicTable {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "list" => to_value(list(parse_args(args)?)?),
            "molar_mass" => to_value(molar_mass(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(PeriodicTable);

/// (序数, 符号, 英文名, 中文名, 原子量, 类别键, 电子层数)
const ELEMENTS: &[(u32, &str, &str, &str, f64, &str, u8)] = &[
    (1, "H", "Hydrogen", "氢", 1.008, "nonmetal", 1),
    (2, "He", "Helium", "氦", 4.0026, "noble", 1),
    (3, "Li", "Lithium", "锂", 6.94, "metal", 2),
    (4, "Be", "Beryllium", "铍", 9.0122, "metal", 2),
    (5, "B", "Boron", "硼", 10.81, "metalloid", 2),
    (6, "C", "Carbon", "碳", 12.011, "nonmetal", 2),
    (7, "N", "Nitrogen", "氮", 14.007, "nonmetal", 2),
    (8, "O", "Oxygen", "氧", 15.999, "nonmetal", 2),
    (9, "F", "Fluorine", "氟", 18.998, "nonmetal", 2),
    (10, "Ne", "Neon", "氖", 20.180, "noble", 2),
    (11, "Na", "Sodium", "钠", 22.990, "metal", 3),
    (12, "Mg", "Magnesium", "镁", 24.305, "metal", 3),
    (13, "Al", "Aluminium", "铝", 26.982, "metal", 3),
    (14, "Si", "Silicon", "硅", 28.085, "metalloid", 3),
    (15, "P", "Phosphorus", "磷", 30.974, "nonmetal", 3),
    (16, "S", "Sulfur", "硫", 32.06, "nonmetal", 3),
    (17, "Cl", "Chlorine", "氯", 35.45, "nonmetal", 3),
    (18, "Ar", "Argon", "氩", 39.948, "noble", 3),
    (19, "K", "Potassium", "钾", 39.098, "metal", 4),
    (20, "Ca", "Calcium", "钙", 40.078, "metal", 4),
    (26, "Fe", "Iron", "铁", 55.845, "metal", 4),
    (27, "Co", "Cobalt", "钴", 58.933, "metal", 4),
    (28, "Ni", "Nickel", "镍", 58.693, "metal", 4),
    (29, "Cu", "Copper", "铜", 63.546, "metal", 4),
    (30, "Zn", "Zinc", "锌", 65.38, "metal", 4),
    (35, "Br", "Bromine", "溴", 79.904, "nonmetal", 4),
    (47, "Ag", "Silver", "银", 107.87, "metal", 5),
    (50, "Sn", "Tin", "锡", 118.71, "metal", 5),
    (53, "I", "Iodine", "碘", 126.90, "nonmetal", 5),
    (74, "W", "Tungsten", "钨", 183.84, "metal", 6),
    (78, "Pt", "Platinum", "铂", 195.08, "metal", 6),
    (79, "Au", "Gold", "金", 196.97, "metal", 6),
    (80, "Hg", "Mercury", "汞", 200.59, "metal", 6),
    (82, "Pb", "Lead", "铅", 207.2, "metal", 6),
    (92, "U", "Uranium", "铀", 238.03, "metal", 7),
];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Element {
    pub number: u32,
    pub symbol: &'static str,
    pub name_en: &'static str,
    pub name_zh: &'static str,
    pub weight: f64,
    /// categories.<key>
    pub category: &'static str,
    pub shells: u8,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListArgs {
    /// 匹配中英文名、符号或序数
    #[serde(default)]
    pub query: String,
    /// categories.<key> 过滤
    #[serde(default)]
    pub category: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListOutput {
    pub elements: Vec<Element>,
    pub total: usize,
}

pub fn list(args: ListArgs) -> PluginResult<ListOutput> {
    let needle = args.query.trim().to_lowercase();
    let elements: Vec<Element> = ELEMENTS
        .iter()
        .filter(|e| args.category.as_ref().is_none_or(|c| e.5 == c))
        .filter(|e| {
            needle.is_empty()
                || e.1.to_lowercase().contains(&needle)
                || e.2.to_lowercase().contains(&needle)
                || e.3.contains(&needle)
                || format!("{}", e.0) == needle
        })
        .map(|e| Element {
            number: e.0,
            symbol: e.1,
            name_en: e.2,
            name_zh: e.3,
            weight: e.4,
            category: e.5,
            shells: e.6,
        })
        .collect();
    if elements.is_empty() && !needle.is_empty() {
        return Err(PluginError::new("element.not_found").with("query", args.query.trim()));
    }
    let total = elements.len();
    Ok(ListOutput { elements, total })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MolarArgs {
    /// 化学式（如 H2O、CuSO4），元素符号区分大小写
    pub formula: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MassPart {
    pub symbol: &'static str,
    pub count: u32,
    pub subtotal: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MolarOutput {
    pub formula: String,
    pub molar_mass: f64,
    pub parts: Vec<MassPart>,
}

/// 化学式解析：元素符号 + 可选数字（如 H2O、CuSO4、NaCl）；括号不支持（首版）
fn parse_formula(formula: &str) -> PluginResult<Vec<(usize, u32)>> {
    let mut chars = formula.chars().peekable();
    let mut out: Vec<(usize, u32)> = Vec::new();
    while let Some(&c) = chars.peek() {
        if !c.is_ascii_uppercase() {
            return Err(PluginError::new("element.bad_formula").with("char", c.to_string()));
        }
        let mut symbol = String::new();
        symbol.push(c);
        chars.next();
        if let Some(&lower) = chars.peek()
            && lower.is_ascii_lowercase()
        {
            symbol.push(lower);
            chars.next();
        }
        let Some((_, _, _, _, _, _, _)) = ELEMENTS.iter().find(|e| e.1 == symbol) else {
            return Err(PluginError::new("element.unknown_symbol").with("symbol", symbol));
        };
        let mut count_text = String::new();
        while let Some(&digit) = chars.peek() {
            if digit.is_ascii_digit() {
                count_text.push(digit);
                chars.next();
            } else {
                break;
            }
        }
        let count: u32 = if count_text.is_empty() {
            1
        } else {
            count_text.parse().unwrap_or(0)
        };
        if count == 0 {
            return Err(PluginError::new("element.bad_count").with("text", count_text));
        }
        let index = ELEMENTS.iter().position(|e| e.1 == symbol).unwrap();
        match out.iter_mut().find(|(i, _)| *i == index) {
            Some((_, n)) => *n += count,
            None => out.push((index, count)),
        }
    }
    if out.is_empty() {
        return Err(PluginError::new("element.bad_formula").with("char", ""));
    }
    Ok(out)
}

pub fn molar_mass(args: MolarArgs) -> PluginResult<MolarOutput> {
    let formula = args.formula.trim().to_owned();
    if formula.is_empty() || formula.len() > 100 {
        return Err(PluginError::new("element.bad_formula").with("char", ""));
    }
    let parts = parse_formula(&formula)?;
    let mut mass = 0.0f64;
    let mut detail = Vec::new();
    for (index, count) in &parts {
        let weight = ELEMENTS[*index].4;
        mass += weight * *count as f64;
        detail.push(MassPart {
            symbol: ELEMENTS[*index].1,
            count: *count,
            subtotal: (weight * *count as f64 * 1000.0).round() / 1000.0,
        });
    }
    Ok(MolarOutput {
        formula: formula.clone(),
        molar_mass: (mass * 1000.0).round() / 1000.0,
        parts: detail,
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
