//! 朝代速查插件后端：从夏到清的朝代表（起止年份、都城、开国/亡国君主）
//! 与公元年份归属查询。数据为公共领域的正史常识，年份按通行断代（含争议年份取主流说法）。
//!
//! 负数表示公元前（如 -221 = 公元前 221 年）。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct DynastyTimeline {
    manifest: Manifest,
}

impl Default for DynastyTimeline {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for DynastyTimeline {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "list" => to_value(list(parse_args(args)?)?),
            "lookup" => to_value(lookup(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(DynastyTimeline);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dynasty {
    /// i18n 键：dynasties.<key>
    pub key: &'static str,
    /// 起始年份（负数为公元前）
    pub start: i32,
    /// 结束年份（负数为公元前）
    pub end: i32,
    /// 都城（i18n 键：capitals.<key>）
    pub capital: &'static str,
    /// 开国之君（i18n 键：rulers.<key>）
    pub founder: &'static str,
    /// 亡国之君（i18n 键：rulers.<key>）
    pub last: &'static str,
}

static DYNASTIES: &[Dynasty] = &[
    Dynasty {
        key: "xia",
        start: -2070,
        end: -1600,
        capital: "yangcheng",
        founder: "yu",
        last: "jie",
    },
    Dynasty {
        key: "shang",
        start: -1600,
        end: -1046,
        capital: "yin",
        founder: "tang",
        last: "zhou",
    },
    Dynasty {
        key: "western-zhou",
        start: -1046,
        end: -771,
        capital: "haojing",
        founder: "wu",
        last: "you",
    },
    Dynasty {
        key: "spring-autumn",
        start: -770,
        end: -476,
        capital: "luoyi",
        founder: "ping",
        last: "jing",
    },
    Dynasty {
        key: "warring-states",
        start: -475,
        end: -221,
        capital: "various",
        founder: "none",
        last: "none",
    },
    Dynasty {
        key: "qin",
        start: -221,
        end: -207,
        capital: "xianyang",
        founder: "shihuang",
        last: "er",
    },
    Dynasty {
        key: "western-han",
        start: -202,
        end: 8,
        capital: "changan",
        founder: "gaozu",
        last: "ping",
    },
    Dynasty {
        key: "xin",
        start: 9,
        end: 23,
        capital: "changan",
        founder: "mang",
        last: "mang",
    },
    Dynasty {
        key: "eastern-han",
        start: 25,
        end: 220,
        capital: "luoyang",
        founder: "guangwu",
        last: "xian",
    },
    Dynasty {
        key: "three-kingdoms",
        start: 220,
        end: 280,
        capital: "various",
        founder: "none",
        last: "none",
    },
    Dynasty {
        key: "western-jin",
        start: 266,
        end: 316,
        capital: "luoyang",
        founder: "wudi",
        last: "huai",
    },
    Dynasty {
        key: "eastern-jin",
        start: 317,
        end: 420,
        capital: "jiankang",
        founder: "yuandi",
        last: "gong",
    },
    Dynasty {
        key: "northern-southern",
        start: 420,
        end: 589,
        capital: "various",
        founder: "none",
        last: "none",
    },
    Dynasty {
        key: "sui",
        start: 581,
        end: 618,
        capital: "daxing",
        founder: "wendi",
        last: "gong",
    },
    Dynasty {
        key: "tang",
        start: 618,
        end: 907,
        capital: "changan",
        founder: "gaozu",
        last: "ai",
    },
    Dynasty {
        key: "five-dynasties",
        start: 907,
        end: 960,
        capital: "various",
        founder: "none",
        last: "none",
    },
    Dynasty {
        key: "northern-song",
        start: 960,
        end: 1127,
        capital: "bianliang",
        founder: "taizu",
        last: "qinzong",
    },
    Dynasty {
        key: "southern-song",
        start: 1127,
        end: 1279,
        capital: "linan",
        founder: "gaozong",
        last: "bing",
    },
    Dynasty {
        key: "yuan",
        start: 1271,
        end: 1368,
        capital: "dadu",
        founder: "shizu",
        last: "shundi",
    },
    Dynasty {
        key: "ming",
        start: 1368,
        end: 1644,
        capital: "beijing",
        founder: "taizu",
        last: "chongzhen",
    },
    Dynasty {
        key: "qing",
        start: 1644,
        end: 1912,
        capital: "beijing",
        founder: "shunzhi",
        last: "xuantong",
    },
];

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListArgs {
    /// 按朝代名过滤（对 key 包含匹配；支持中文——前端传 locales 里的名字或拼音 key 均可）
    #[serde(default)]
    pub query: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListOutput {
    pub dynasties: Vec<Dynasty>,
    pub total: usize,
}

pub fn list(args: ListArgs) -> PluginResult<ListOutput> {
    let needle = args.query.trim().to_lowercase();
    let dynasties: Vec<Dynasty> = DYNASTIES
        .iter()
        .filter(|d| needle.is_empty() || d.key.contains(&needle))
        .cloned()
        .collect();
    if dynasties.is_empty() && !needle.is_empty() {
        return Err(PluginError::new("dynasty.not_found").with("query", args.query.trim()));
    }
    let total = dynasties.len();
    Ok(ListOutput { dynasties, total })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupArgs {
    /// 公元年份；负数表示公元前（如 -221）
    pub year: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupOutput {
    pub year: i32,
    /// 公元前/公元显示辅助
    pub bce: bool,
    pub matches: Vec<Dynasty>,
}

pub fn lookup(args: LookupArgs) -> PluginResult<LookupOutput> {
    if args.year == 0 || args.year < -2100 || args.year > 2100 {
        return Err(PluginError::new("dynasty.bad_year").with("year", args.year));
    }
    let matches: Vec<Dynasty> = DYNASTIES
        .iter()
        .filter(|d| args.year >= d.start && args.year <= d.end)
        .cloned()
        .collect();
    Ok(LookupOutput {
        year: args.year,
        bce: args.year < 0,
        matches,
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
