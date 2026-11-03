//! 纸张尺寸速查插件后端：ISO A/B/C 系列、国产 K 系列、常见信封与照片尺寸，
//! 以及毫米/英寸/像素按 DPI 换算。数据内置，全部离线。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct PaperSize {
    manifest: Manifest,
}

impl Default for PaperSize {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for PaperSize {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "list" => to_value(list(parse_args(args)?)?),
            "convert" => to_value(convert(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(PaperSize);

/// 尺寸条目：名称（i18n 键）、短边×长边（毫米）、类别、说明键
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Size {
    /// i18n 键：sizes.<key>
    pub key: &'static str,
    /// 短边（毫米）
    pub short: f64,
    /// 长边（毫米）
    pub long: f64,
    /// 分类（i18n 键：categories.<key>）
    pub category: &'static str,
}

static SIZES: &[Size] = &[
    // ISO A 系列（毫米，4 舍 5 入到 1 位）
    Size {
        key: "a0",
        short: 841.0,
        long: 1189.0,
        category: "iso",
    },
    Size {
        key: "a1",
        short: 594.0,
        long: 841.0,
        category: "iso",
    },
    Size {
        key: "a2",
        short: 420.0,
        long: 594.0,
        category: "iso",
    },
    Size {
        key: "a3",
        short: 297.0,
        long: 420.0,
        category: "iso",
    },
    Size {
        key: "a4",
        short: 210.0,
        long: 297.0,
        category: "iso",
    },
    Size {
        key: "a5",
        short: 148.0,
        long: 210.0,
        category: "iso",
    },
    Size {
        key: "a6",
        short: 105.0,
        long: 148.0,
        category: "iso",
    },
    Size {
        key: "a7",
        short: 74.0,
        long: 105.0,
        category: "iso",
    },
    Size {
        key: "a8",
        short: 52.0,
        long: 74.0,
        category: "iso",
    },
    // ISO B 系列
    Size {
        key: "b0",
        short: 1000.0,
        long: 1414.0,
        category: "iso",
    },
    Size {
        key: "b1",
        short: 707.0,
        long: 1000.0,
        category: "iso",
    },
    Size {
        key: "b2",
        short: 500.0,
        long: 707.0,
        category: "iso",
    },
    Size {
        key: "b3",
        short: 353.0,
        long: 500.0,
        category: "iso",
    },
    Size {
        key: "b4",
        short: 250.0,
        long: 353.0,
        category: "iso",
    },
    Size {
        key: "b5",
        short: 176.0,
        long: 250.0,
        category: "iso",
    },
    Size {
        key: "b6",
        short: 125.0,
        long: 176.0,
        category: "iso",
    },
    // ISO C 系列（信封）
    Size {
        key: "c0",
        short: 917.0,
        long: 1297.0,
        category: "iso",
    },
    Size {
        key: "c4",
        short: 229.0,
        long: 324.0,
        category: "iso",
    },
    Size {
        key: "c5",
        short: 162.0,
        long: 229.0,
        category: "iso",
    },
    Size {
        key: "c6",
        short: 114.0,
        long: 162.0,
        category: "iso",
    },
    // 国产 K 系列（大度/正度，常见全开对折数）
    Size {
        key: "k8-dadu",
        short: 285.0,
        long: 420.0,
        category: "k",
    },
    Size {
        key: "k16-dadu",
        short: 210.0,
        long: 285.0,
        category: "k",
    },
    Size {
        key: "k32-dadu",
        short: 142.5,
        long: 210.0,
        category: "k",
    },
    Size {
        key: "k8-zhengdu",
        short: 260.0,
        long: 370.0,
        category: "k",
    },
    Size {
        key: "k16-zhengdu",
        short: 185.0,
        long: 260.0,
        category: "k",
    },
    Size {
        key: "k32-zhengdu",
        short: 130.0,
        long: 185.0,
        category: "k",
    },
    // 常用信封
    Size {
        key: "env-dl",
        short: 110.0,
        long: 220.0,
        category: "envelope",
    },
    Size {
        key: "env-c5",
        short: 162.0,
        long: 229.0,
        category: "envelope",
    },
    Size {
        key: "env-c6",
        short: 114.0,
        long: 162.0,
        category: "envelope",
    },
    Size {
        key: "env-5",
        short: 110.0,
        long: 220.0,
        category: "envelope",
    },
    Size {
        key: "env-7",
        short: 162.0,
        long: 229.0,
        category: "envelope",
    },
    // 照片与证件
    Size {
        key: "photo-1in",
        short: 25.0,
        long: 35.0,
        category: "photo",
    },
    Size {
        key: "photo-2in",
        short: 35.0,
        long: 49.0,
        category: "photo",
    },
    Size {
        key: "photo-id",
        short: 33.0,
        long: 48.0,
        category: "photo",
    },
    Size {
        key: "photo-5in",
        short: 89.0,
        long: 127.0,
        category: "photo",
    },
    Size {
        key: "photo-6in",
        short: 102.0,
        long: 152.0,
        category: "photo",
    },
    Size {
        key: "photo-7in",
        short: 127.0,
        long: 178.0,
        category: "photo",
    },
];

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListArgs {
    /// 按名称/类别过滤（对 key 与 category 做包含匹配）
    #[serde(default)]
    pub query: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListOutput {
    pub sizes: Vec<Size>,
    pub total: usize,
}

pub fn list(args: ListArgs) -> PluginResult<ListOutput> {
    let needle = args.query.trim().to_lowercase();
    let sizes: Vec<Size> = SIZES
        .iter()
        .filter(|size| {
            needle.is_empty() || size.key.contains(&needle) || size.category.contains(&needle)
        })
        .cloned()
        .collect();
    if sizes.is_empty() && !needle.is_empty() {
        return Err(PluginError::new("paper.not_found").with("query", args.query.trim()));
    }
    let total = sizes.len();
    Ok(ListOutput { sizes, total })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertArgs {
    /// 毫米数
    pub mm: f64,
    /// DPI（默认 300）
    #[serde(default)]
    pub dpi: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertOutput {
    pub mm: f64,
    pub inches: f64,
    /// 换算为 1:1 像素（按 DPI）
    pub pixels: u64,
    pub dpi: f64,
}

pub fn convert(args: ConvertArgs) -> PluginResult<ConvertOutput> {
    let mm = args.mm;
    if mm <= 0.0 {
        return Err(PluginError::new("paper.positive_required").with("name", "mm"));
    }
    let dpi = args.dpi.unwrap_or(300.0);
    if dpi <= 0.0 {
        return Err(PluginError::new("paper.positive_required").with("name", "dpi"));
    }
    Ok(ConvertOutput {
        mm,
        inches: mm / 25.4,
        pixels: (mm / 25.4 * dpi).round() as u64,
        dpi,
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
