//! 命令行 / Agent 用的扁平入口：用一行文字描述一件物品，按人数与天数算出要带的数量与估算重量。
//!
//! 物品写法：`名称:数量[:规则][:单件克数]`。规则为空表示共享（不翻倍），`p` 按人数、
//! `d` 按天数、`pd` 两者都乘。内部调用界面同一套模板生成规则（packing::generate）。

use serde::{Deserialize, Serialize};
use tf_plugin_api::PluginResult;

use crate::packing::{GenerateArgs, TemplateItem, errp, generate, grams};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistArgs {
    pub items: Vec<String>,
    #[serde(default = "one")]
    pub people: i64,
    #[serde(default = "one")]
    pub days: i64,
}

fn one() -> i64 {
    1
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub name: String,
    pub quantity: i64,
    /// 本项总重（克的可读文本；未给单件重量时为空）
    pub weight: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistOut {
    pub items: Vec<Row>,
    /// 已知重量的合计
    pub total_weight: String,
    /// 没有填写单件重量的物品数
    pub unknown_weight_items: usize,
}

fn parse(line: &str) -> PluginResult<TemplateItem> {
    let bad = || errp("plc.bad_line", "value", line);
    let parts: Vec<&str> = line.split(':').map(str::trim).collect();
    if parts.len() < 2 || parts.len() > 4 || parts[0].is_empty() {
        return Err(bad());
    }
    let base: i64 = parts[1].parse().map_err(|_| bad())?;
    let rule = parts
        .get(2)
        .copied()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !matches!(rule.as_str(), "" | "p" | "d" | "pd" | "dp") {
        return Err(bad());
    }
    let weight = match parts.get(3) {
        Some(w) if !w.is_empty() => Some(w.parse::<i64>().map_err(|_| bad())?),
        _ => None,
    };
    Ok(TemplateItem {
        name: parts[0].to_string(),
        category: String::new(),
        unit: String::new(),
        base,
        per_person: rule.contains('p'),
        per_day: rule.contains('d'),
        note: None,
        weight,
    })
}

pub fn checklist(args: ChecklistArgs) -> PluginResult<ChecklistOut> {
    if args.items.is_empty() {
        return Err(errp("plc.empty_name", "index", 0));
    }
    let template = args
        .items
        .iter()
        .map(|l| parse(l))
        .collect::<PluginResult<Vec<_>>>()?;
    let items = generate(GenerateArgs {
        template,
        people: args.people,
        days: args.days,
    })?;
    let mut total = 0i64;
    let mut unknown = 0usize;
    let rows = items
        .into_iter()
        .map(|item| {
            let weight = match item.weight {
                Some(w) => {
                    let sum = w.saturating_mul(item.target);
                    total = total.saturating_add(sum);
                    grams(sum)
                }
                None => {
                    if item.target > 0 {
                        unknown += 1;
                    }
                    String::new()
                }
            };
            Row {
                name: item.name,
                quantity: item.target,
                weight,
            }
        })
        .collect();
    Ok(ChecklistOut {
        items: rows,
        total_weight: grams(total),
        unknown_weight_items: unknown,
    })
}

#[cfg(test)]
#[path = "flat_test.rs"]
mod tests;
