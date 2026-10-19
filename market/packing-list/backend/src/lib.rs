//! 旅行行李清单（在线安装插件）：模板按人数/天数生成数量、打包进度与重量汇总
//! 全部在 Rust 侧以整数（克/件）计算；行程与模板由前端经 usePluginState
//! 持久化到 ToolForge SQLite，后端只做无状态计算与导出。

mod packing;

use std::sync::atomic::{AtomicU64, Ordering};

use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

use packing::{Summary, Trip, grams};

const MANIFEST: &str = include_str!("../../manifest.json");

static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportArgs {
    format: Format,
    summary: Value,
    trip_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Format {
    Json,
    Text,
}

pub struct PackingList {
    manifest: Manifest,
}

impl Default for PackingList {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

fn new_id() -> PluginResult<Value> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let counter = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    to_value(json!({ "id": format!("p{nanos:012x}{counter:02x}") }))
}

/// 未装清单文本：按类别分组的待装项
fn unpacked_text(summary: &Summary) -> String {
    let mut out = String::new();
    if summary.unpacked.is_empty() {
        out.push_str("All packed!\n");
        return out;
    }
    let mut current = String::new();
    for item in &summary.unpacked {
        if item.category != current {
            current = item.category.clone();
            out.push_str(&format!(
                "\n[{}]\n",
                if current.is_empty() { "-" } else { &current }
            ));
        }
        out.push_str(&format!(
            "- {} × {}{}\n",
            item.name,
            item.remaining,
            item.owner
                .as_ref()
                .map(|owner| format!(" ({owner})"))
                .unwrap_or_default(),
        ));
    }
    out
}

fn markdown(summary: &Summary, trip_name: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Packing list: {trip_name}\n\n"));
    out.push_str(&format!(
        "- Packed: {}/{} (excluded {})\n",
        summary.packed, summary.target, summary.excluded
    ));
    if summary.unknown_weight_items > 0 {
        out.push_str(&format!(
            "- Weight: {} (incomplete: {} items without weight)\n",
            grams(summary.total_weight),
            summary.unknown_weight_items
        ));
    } else {
        out.push_str(&format!("- Weight: {}\n", grams(summary.total_weight)));
    }
    if !summary.bags.is_empty() {
        out.push_str("\n## Bags\n\n");
        for bag in &summary.bags {
            match bag.total_weight {
                Some(total) => {
                    out.push_str(&format!(
                        "- {} ({}): {} + self {}\n",
                        if bag.name.is_empty() {
                            &bag.id
                        } else {
                            &bag.name
                        },
                        bag.id,
                        grams(bag.packed_weight),
                        grams(bag.self_weight.unwrap_or_default()),
                    ));
                    let _ = total;
                }
                None => {
                    out.push_str(&format!(
                        "- {} ({}): {} (self weight unknown)\n",
                        if bag.name.is_empty() {
                            &bag.id
                        } else {
                            &bag.name
                        },
                        bag.id,
                        grams(bag.packed_weight),
                    ));
                }
            }
        }
    }
    out.push_str("\n## To pack\n\n");
    out.push_str(&unpacked_text(summary));
    out
}

impl PackingList {
    fn export(&self, args: ExportArgs) -> PluginResult<Value> {
        let summary: Summary = serde_json::from_value(args.summary)
            .map_err(|e| PluginError::new("plc.serialize").with("detail", e.to_string()))?;
        let (content, ext) = match args.format {
            Format::Json => (
                serde_json::to_string_pretty(&summary)
                    .map_err(|e| PluginError::new("plc.serialize").with("detail", e.to_string()))?,
                "json",
            ),
            Format::Text => (unpacked_text(&summary), "txt"),
        };
        to_value(
            json!({ "content": content, "ext": ext, "markdown": markdown(&summary, &args.trip_name) }),
        )
    }
}

impl ToolPlugin for PackingList {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "new_id" => new_id(),
            "generate" => {
                let args: packing::GenerateArgs = parse_args(args)?;
                to_value(packing::generate(args)?)
            }
            "summary" => {
                let args: Trip = parse_args(args)?;
                to_value(packing::summarize(&args)?)
            }
            "export" => self.export(parse_args(args)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(PackingList);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
