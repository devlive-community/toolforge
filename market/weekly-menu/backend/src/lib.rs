//! 菜谱与一周菜单后端：份数缩放（含 g/kg、ml/L 同维换算）、一周菜单结构与
//! 购物清单合并（同名同单位合计；适量等原文保留）。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct WeeklyMenu {
    manifest: Manifest,
}

impl Default for WeeklyMenu {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for WeeklyMenu {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "scale" => to_value(scale(parse_args(args)?)?),
            "shopping" => to_value(shopping(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(WeeklyMenu);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScaleArgs {
    /// 基础份数
    pub base_servings: f64,
    /// 目标份数
    pub servings: f64,
    /// 食材：名称 / 数量 / 单位
    pub ingredients: Vec<Ingredient>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ingredient {
    pub name: String,
    pub amount: f64,
    pub unit: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScaledIngredient {
    pub name: String,
    pub amount: f64,
    pub unit: String,
    /// true 表示「适量/少许」原文保留，不乘倍数
    pub as_is: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScaleOutput {
    pub servings: f64,
    pub ingredients: Vec<ScaledIngredient>,
}

const VAGUE: &[&str] = &["适量", "少许", "若干", "as needed"];

/// 缩放：数量乘倍数；同维单位换算为可读形式；适量保留
pub fn scale(args: ScaleArgs) -> PluginResult<ScaleOutput> {
    if !args.base_servings.is_finite()
        || args.base_servings <= 0.0
        || !args.servings.is_finite()
        || args.servings <= 0.0
    {
        return Err(PluginError::new("menu.bad_servings"));
    }
    let factor = args.servings / args.base_servings;
    let mut ingredients = Vec::new();
    for item in &args.ingredients {
        let name = item.name.trim().to_owned();
        if name.is_empty() {
            return Err(PluginError::new("menu.bad_ingredient"));
        }
        if VAGUE.contains(&item.unit.as_str()) || item.unit.is_empty() {
            ingredients.push(ScaledIngredient {
                name,
                amount: item.amount,
                unit: item.unit.clone(),
                as_is: true,
            });
            continue;
        }
        let amount = round2(item.amount * factor);
        let (amount, unit) = convert(amount, &item.unit);
        ingredients.push(ScaledIngredient {
            name,
            amount,
            unit,
            as_is: false,
        });
    }
    Ok(ScaleOutput {
        servings: args.servings,
        ingredients,
    })
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// 同维单位换算（g/kg、ml/L），返回更可读的单位
fn convert(amount: f64, unit: &str) -> (f64, String) {
    match unit {
        "g" if amount >= 1000.0 => (round2(amount / 1000.0), "kg".into()),
        "kg" if amount < 1.0 => (round2(amount * 1000.0), "g".into()),
        "ml" if amount >= 1000.0 => (round2(amount / 1000.0), "L".into()),
        "L" | "l" if amount < 1.0 => (round2(amount * 1000.0), "ml".into()),
        other => (amount, other.to_owned()),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MealSlot {
    /// 菜谱 id（可空 = 外食/留空）
    #[serde(default)]
    pub recipe_id: Option<String>,
    pub servings: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recipe {
    pub id: String,
    pub name: String,
    pub base_servings: f64,
    pub ingredients: Vec<Ingredient>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShoppingArgs {
    /// 一周菜单：7 天 × 3 餐（索引 0..20；0 周一早 … 20 周日晚）
    pub menu: Vec<MealSlot>,
    pub recipes: Vec<Recipe>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShoppingItem {
    pub name: String,
    pub amount: f64,
    pub unit: String,
    /// 来源菜谱名（同名同单位合并）
    pub from: Vec<String>,
    pub as_is: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShoppingOutput {
    pub items: Vec<ShoppingItem>,
    pub meals: usize,
}

/// 购物清单：遍历菜单非空餐次，按份数缩放后同名同单位合计
pub fn shopping(args: ShoppingArgs) -> PluginResult<ShoppingOutput> {
    if args.menu.len() > 21 {
        return Err(PluginError::new("menu.too_many_slots").with("max", 21));
    }
    let mut merged: std::collections::HashMap<(String, String), ShoppingItem> =
        std::collections::HashMap::new();
    let mut meals = 0usize;
    for slot in &args.menu {
        let Some(recipe_id) = &slot.recipe_id else {
            continue;
        };
        let Some(recipe) = args.recipes.iter().find(|r| &r.id == recipe_id) else {
            return Err(PluginError::new("menu.unknown_recipe").with("id", recipe_id.as_str()));
        };
        if !slot.servings.is_finite() || slot.servings <= 0.0 {
            return Err(PluginError::new("menu.bad_servings"));
        }
        meals += 1;
        let factor = slot.servings / recipe.base_servings;
        for ingredient in &recipe.ingredients {
            let vague = VAGUE.contains(&ingredient.unit.as_str()) || ingredient.unit.is_empty();
            let (amount, unit, as_is) = if vague {
                (ingredient.amount, ingredient.unit.clone(), true)
            } else {
                let (amount, unit) = convert(round2(ingredient.amount * factor), &ingredient.unit);
                (amount, unit, false)
            };
            let key = (
                ingredient.name.trim().to_lowercase(),
                if as_is {
                    ingredient.unit.clone()
                } else {
                    unit.clone()
                },
            );
            let entry = merged
                .entry((
                    ingredient.name.trim().to_owned(),
                    if vague {
                        ingredient.unit.clone()
                    } else {
                        unit.clone()
                    },
                ))
                .or_insert_with(|| ShoppingItem {
                    name: ingredient.name.trim().to_owned(),
                    amount: 0.0,
                    unit: if vague { ingredient.unit.clone() } else { unit },
                    from: Vec::new(),
                    as_is,
                });
            if !as_is {
                entry.amount += amount;
            }
            if !entry.from.contains(&recipe.name) {
                entry.from.push(recipe.name.clone());
            }
            let _ = key;
        }
    }
    let mut items: Vec<ShoppingItem> = merged.into_values().collect();
    items.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(ShoppingOutput { items, meals })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
