//! 衣橱手账后端：衣物/穿搭/穿着记录的统计与校验（状态由前端经
//! plugin_state 持久化），Rust 负责可穿性筛选、搭配生成与使用统计。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct WardrobeJournal {
    manifest: Manifest,
}

impl Default for WardrobeJournal {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for WardrobeJournal {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "suggest" => to_value(suggest(parse_args(args)?)?),
            "stats" => to_value(stats(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(WardrobeJournal);

/// 衣物（前端状态传入）
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub name: String,
    /// top / bottom / shoes / outer / accessory
    pub category: String,
    /// 颜色键
    #[serde(default)]
    pub color: Option<String>,
    /// 季节标签（spring/summer/autumn/winter/all）
    #[serde(default)]
    pub season: Option<String>,
    /// clean / dirty（待洗）
    #[serde(default)]
    pub state: Option<String>,
    /// 最近穿着日期（YYYY-MM-DD，可选）
    #[serde(default)]
    pub last_worn: Option<String>,
    #[serde(default)]
    pub wear_count: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outfit {
    pub id: String,
    pub name: String,
    /// 单品 id 集合
    pub item_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestArgs {
    pub items: Vec<Item>,
    pub outfits: Vec<Outfit>,
    /// 期望季节（可空 = 全部）
    #[serde(default)]
    pub season: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub outfit_id: String,
    pub name: String,
    /// 可穿性：ready（全部可穿）/ has_dirty（含待洗单品）
    pub status: &'static str,
    /// 待洗单品名
    pub dirty: Vec<String>,
}

/// 搭配建议：按季节筛选（all 匹配一切），标记含待洗单品的组合
pub fn suggest(args: SuggestArgs) -> PluginResult<SuggestList> {
    if args.outfits.is_empty() {
        return Err(PluginError::new("wardrobe.no_outfits"));
    }
    let find = |id: &str| args.items.iter().find(|item| item.id == id);
    let mut suggestions = Vec::new();
    for outfit in &args.outfits {
        let mut dirty = Vec::new();
        let mut season_ok = true;
        for item_id in &outfit.item_ids {
            let Some(item) = find(item_id) else { continue };
            if item.state.as_deref() == Some("dirty") {
                dirty.push(item.name.clone());
            }
            if let Some(season) = &args.season {
                let item_season = item.season.as_deref().unwrap_or("all");
                if item_season != "all" && item_season != season {
                    season_ok = false;
                }
            }
        }
        let status = if dirty.is_empty() {
            "ready"
        } else {
            "has_dirty"
        };
        if !season_ok {
            continue;
        }
        suggestions.push(Suggestion {
            outfit_id: outfit.id.clone(),
            name: outfit.name.clone(),
            status,
            dirty,
        });
    }
    if suggestions.is_empty() {
        return Err(PluginError::new("wardrobe.no_match")
            .with("season", args.season.clone().unwrap_or_default()));
    }
    Ok(SuggestList { suggestions })
}

/// suggest 实际返回列表
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestList {
    pub suggestions: Vec<Suggestion>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsArgs {
    pub items: Vec<Item>,
    pub outfits: Vec<Outfit>,
    /// 穿着记录：outfit id → 次数
    #[serde(default)]
    pub wear_log: std::collections::HashMap<String, u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemStat {
    pub id: String,
    pub name: String,
    pub wear_count: u64,
    /// 参与的搭配名
    pub outfits: Vec<String>,
    pub state: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsOutput {
    pub items: Vec<ItemStat>,
    pub dirty_count: usize,
    pub outfit_count: usize,
}

/// 单品使用统计：穿着次数（按所在搭配的穿着次数聚合）与参与的搭配
pub fn stats(args: StatsArgs) -> PluginResult<StatsOutput> {
    let mut items = Vec::new();
    let mut dirty_count = 0usize;
    for item in &args.items {
        let mut wear_count = 0u64;
        let mut outfit_names = Vec::new();
        for outfit in &args.outfits {
            if outfit.item_ids.contains(&item.id) {
                outfit_names.push(outfit.name.clone());
                wear_count += args.wear_log.get(&outfit.id).copied().unwrap_or(0);
            }
        }
        let state = item.state.clone().unwrap_or_else(|| "clean".into());
        if state == "dirty" {
            dirty_count += 1;
        }
        items.push(ItemStat {
            id: item.id.clone(),
            name: item.name.clone(),
            wear_count,
            outfits: outfit_names,
            state,
        });
    }
    items.sort_by(|a, b| b.wear_count.cmp(&a.wear_count).then(a.name.cmp(&b.name)));
    Ok(StatsOutput {
        items,
        dirty_count,
        outfit_count: args.outfits.len(),
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
