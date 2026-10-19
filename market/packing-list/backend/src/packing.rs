//! 行李清单计算：模板按人数/天数生成数量、打包进度与重量汇总。
//! 数量与重量全部整数（克），非法输入返回错误码交由前端翻译；
//! 未填重量的物品单独计数，不把未知当零。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::PluginError;

/// 单项数量上限
pub const MAX_QUANTITY: i64 = 100_000;
/// 单件重量上限（克）：500kg
pub const MAX_WEIGHT: i64 = 500_000;
/// 单个行程物品数上限
pub const MAX_ITEMS: usize = 1000;
/// 箱包数上限
pub const MAX_BAGS: usize = 32;
/// 天数/人数上限
pub const MAX_SPAN: i64 = 365;

pub fn errp(code: &str, key: &str, value: impl Into<Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

// ---- 模板 ----

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TemplateItem {
    pub name: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub unit: String,
    /// 基础数量
    pub base: i64,
    #[serde(default)]
    pub per_person: bool,
    #[serde(default)]
    pub per_day: bool,
    #[serde(default)]
    pub note: Option<String>,
    /// 单件重量（克，可选）
    #[serde(default)]
    pub weight: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateArgs {
    pub template: Vec<TemplateItem>,
    pub people: i64,
    pub days: i64,
}

// ---- 行程 ----

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Bag {
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// 箱包自重（克，可选）
    #[serde(default)]
    pub self_weight: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TripItem {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub unit: String,
    pub target: i64,
    #[serde(default)]
    pub packed: i64,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub bag_id: Option<String>,
    /// 标记「不带」：不计入打包进度
    #[serde(default)]
    pub excluded: bool,
    /// 单件重量（克，可选）
    #[serde(default)]
    pub weight: Option<i64>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trip {
    pub people: i64,
    pub days: i64,
    pub bags: Vec<Bag>,
    pub items: Vec<TripItem>,
}

// ---- 输出 ----

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CategoryProgress {
    pub category: String,
    pub target: i64,
    pub packed: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BagWeight {
    pub id: String,
    pub name: String,
    /// 已装物品重量（克）
    pub packed_weight: i64,
    /// 箱包自重（克；未填为 null）
    pub self_weight: Option<i64>,
    /// 自重 + 已装重量；自重未知时为 null
    pub total_weight: Option<i64>,
    /// 已装但重量未知的物品数
    pub unknown_weight_items: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UnpackedItem {
    pub id: String,
    pub name: String,
    pub category: String,
    pub remaining: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bag_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub target: i64,
    pub packed: i64,
    pub excluded: i64,
    /// 已装但重量未知的物品数（不完整估算）
    pub unknown_weight_items: i64,
    /// 重量已知的总重（克）
    pub total_weight: i64,
    pub categories: Vec<CategoryProgress>,
    pub bags: Vec<BagWeight>,
    pub unpacked: Vec<UnpackedItem>,
}

fn validate_span(people: i64, days: i64) -> Result<(), PluginError> {
    if !(1..=MAX_SPAN).contains(&people) {
        return Err(errp("plc.people_range", "max", MAX_SPAN));
    }
    if !(1..=MAX_SPAN).contains(&days) {
        return Err(errp("plc.days_range", "max", MAX_SPAN));
    }
    Ok(())
}

fn validate_weight(value: i64, code: &str) -> Result<(), PluginError> {
    if !(0..=MAX_WEIGHT).contains(&value) {
        return Err(errp(code, "max", MAX_WEIGHT));
    }
    Ok(())
}

/// 模板 → 本次物品：按人数/天数应用数量规则，共享物品不自动翻倍
pub fn generate(args: GenerateArgs) -> Result<Vec<TripItem>, PluginError> {
    validate_span(args.people, args.days)?;
    if args.template.len() > MAX_ITEMS {
        return Err(errp("plc.too_many_items", "max", MAX_ITEMS));
    }
    let mut items = Vec::with_capacity(args.template.len());
    for (index, item) in args.template.iter().enumerate() {
        if item.name.trim().is_empty() {
            return Err(errp("plc.empty_name", "index", index));
        }
        let base = item.base;
        if !(0..=MAX_QUANTITY).contains(&base) {
            return Err(errp("plc.quantity_range", "max", MAX_QUANTITY));
        }
        let mut target = base;
        if item.per_person {
            target *= args.people;
        }
        if item.per_day {
            target *= args.days;
        }
        if target > MAX_QUANTITY {
            return Err(errp("plc.quantity_range", "max", MAX_QUANTITY));
        }
        if let Some(weight) = item.weight {
            validate_weight(weight, "plc.weight_range")?;
        }
        items.push(TripItem {
            id: format!("tpl-{index}"),
            name: item.name.trim().to_owned(),
            category: item.category.trim().to_owned(),
            unit: item.unit.trim().to_owned(),
            target,
            packed: 0,
            owner: None,
            bag_id: None,
            excluded: false,
            weight: item.weight,
            note: item.note.clone(),
        });
    }
    Ok(items)
}

/// 打包进度与重量汇总
pub fn summarize(trip: &Trip) -> Result<Summary, PluginError> {
    validate_span(trip.people, trip.days)?;
    if trip.bags.len() > MAX_BAGS {
        return Err(errp("plc.too_many_bags", "max", MAX_BAGS));
    }
    if trip.items.len() > MAX_ITEMS {
        return Err(errp("plc.too_many_items", "max", MAX_ITEMS));
    }
    for item in &trip.items {
        if item.target < 0 || item.target > MAX_QUANTITY {
            return Err(errp("plc.quantity_range", "max", MAX_QUANTITY));
        }
        if !(0..=item.target).contains(&item.packed) {
            return Err(errp("plc.packed_range", "max", item.target));
        }
        if let Some(weight) = item.weight {
            validate_weight(weight, "plc.weight_range")?;
        }
        if let Some(bag_id) = &item.bag_id
            && !trip.bags.iter().any(|bag| &bag.id == bag_id)
        {
            return Err(errp("plc.unknown_bag", "id", bag_id.as_str()));
        }
    }
    for bag in &trip.bags {
        if let Some(self_weight) = bag.self_weight {
            validate_weight(self_weight, "plc.weight_range")?;
        }
    }

    let mut target_total = 0i64;
    let mut packed_total = 0i64;
    let mut excluded = 0i64;
    let mut categories: Vec<CategoryProgress> = Vec::new();
    let mut unknown_weight_items = 0i64;
    let mut total_weight = 0i64;
    let mut unpacked = Vec::new();

    // 顺序分类累加（保持首次出现顺序）
    for item in &trip.items {
        if item.excluded {
            excluded += 1;
            continue;
        }
        target_total += item.target;
        packed_total += item.packed;
        if let Some(weight) = item.weight {
            total_weight += weight * item.packed;
        } else if item.packed > 0 {
            unknown_weight_items += 1;
        }
        if item.packed < item.target {
            unpacked.push(UnpackedItem {
                id: item.id.clone(),
                name: item.name.clone(),
                category: item.category.clone(),
                remaining: item.target - item.packed,
                owner: item.owner.clone(),
                bag_id: item.bag_id.clone(),
            });
        }
        match categories
            .iter_mut()
            .find(|entry| entry.category == item.category)
        {
            Some(entry) => {
                entry.target += item.target;
                entry.packed += item.packed;
            }
            None => categories.push(CategoryProgress {
                category: if item.category.is_empty() {
                    "-".to_owned()
                } else {
                    item.category.clone()
                },
                target: item.target,
                packed: item.packed,
            }),
        }
    }

    let mut bags = Vec::with_capacity(trip.bags.len());
    for bag in &trip.bags {
        let mut packed_weight = 0i64;
        let mut unknown = 0i64;
        for item in &trip.items {
            if item.excluded || item.bag_id.as_deref() != Some(bag.id.as_str()) {
                continue;
            }
            match item.weight {
                Some(weight) => packed_weight += weight * item.packed,
                None if item.packed > 0 => unknown += 1,
                None => {}
            }
        }
        let total = bag
            .self_weight
            .map(|self_weight| self_weight + packed_weight);
        bags.push(BagWeight {
            id: bag.id.clone(),
            name: bag.name.clone(),
            packed_weight,
            self_weight: bag.self_weight,
            total_weight: total,
            unknown_weight_items: unknown,
        });
    }

    Ok(Summary {
        target: target_total,
        packed: packed_total,
        excluded,
        unknown_weight_items,
        total_weight,
        categories,
        bags,
        unpacked,
    })
}

/// 重量格式化：克 → "1200 g" / "1.200 kg"
pub fn grams(value: i64) -> String {
    if value >= 1000 {
        format!("{}.{:03} kg", value / 1000, value % 1000)
    } else {
        format!("{value} g")
    }
}

#[cfg(test)]
#[path = "packing_test.rs"]
mod tests;
