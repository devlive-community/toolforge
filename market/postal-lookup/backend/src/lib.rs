//! 邮编区号查询后端：主要城市邮编与电话区号离线双向查询（含别名）。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct PostalLookup {
    manifest: Manifest,
}

impl Default for PostalLookup {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for PostalLookup {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "lookup" => to_value(lookup(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(PostalLookup);

/// (城市中文, 城市/省英文, 邮编, 区号, 行政区划码, 别名)
type CityRow = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
);

const CITIES: &[CityRow] = &[
    ("北京", "Beijing", "100000", "010", "110000", &["京津"]),
    ("上海", "Shanghai", "200000", "021", "310000", &["沪"]),
    ("广州", "Guangzhou", "510000", "020", "440100", &[]),
    ("深圳", "Shenzhen", "518000", "0755", "440300", &[]),
    ("天津", "Tianjin", "300000", "022", "120000", &[]),
    ("重庆", "Chongqing", "400000", "023", "500000", &["渝"]),
    (
        "杭州",
        "Hangzhou",
        "310000",
        "0571",
        "330100",
        &["余杭", "萧山"],
    ),
    ("南京", "Nanjing", "210000", "025", "320100", &["金陵"]),
    ("武汉", "Wuhan", "430000", "027", "420100", &["江城"]),
    ("成都", "Chengdu", "610000", "028", "510100", &["蓉城"]),
    ("西安", "Xi'an", "710000", "029", "610100", &["长安"]),
    ("沈阳", "Shenyang", "110000", "024", "210100", &[]),
    ("哈尔滨", "Harbin", "150000", "0451", "230100", &["冰城"]),
    ("郑州", "Zhengzhou", "450000", "0371", "410100", &[]),
    ("长沙", "Changsha", "410000", "0731", "430100", &["星城"]),
    ("昆明", "Kunming", "650000", "0871", "530100", &["春城"]),
    ("厦门", "Xiamen", "361000", "0592", "350200", &["鹭岛"]),
    ("青岛", "Qingdao", "266000", "0532", "370200", &["琴岛"]),
    ("大连", "Dalian", "116000", "0411", "210200", &[]),
    ("苏州", "Suzhou", "215000", "0512", "320500", &["姑苏"]),
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    /// 邮编 / 区号 / 城市（中英文与别名均可）
    pub query: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CityResult {
    pub city_zh: &'static str,
    pub city_en: &'static str,
    pub postal_code: &'static str,
    pub area_code: &'static str,
    pub admin_code: &'static str,
    /// 命中方式：city / postal / area
    pub matched_by: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub results: Vec<CityResult>,
    pub total: usize,
}

pub fn lookup(args: Args) -> PluginResult<Output> {
    let needle = args.query.trim().to_lowercase();
    if needle.is_empty() {
        return Err(PluginError::new("postal.empty_query"));
    }
    let mut results: Vec<CityResult> = Vec::new();
    let mut matched_by = "";
    for city in CITIES {
        let (zh, en, postal, area, code, aliases) =
            (city.0, city.1, city.2, city.3, city.4, city.5);
        let hit = if needle == *postal || needle == format!("{}000", &postal[..3]) {
            matched_by = "postal";
            true
        } else if needle == *area
            || needle == format!("{}0", &area[..area.len() - 1])
            || (area.len() == 3 && needle == format!("{}00", &area[..1]))
        {
            matched_by = "area";
            true
        } else {
            false
        };
        if hit
            || zh.contains(&needle)
            || en.to_lowercase().contains(&needle)
            || aliases.iter().any(|a| a.to_lowercase().contains(&needle))
        {
            if matched_by.is_empty() {
                matched_by = "city";
            }
            results.push(CityResult {
                city_zh: zh,
                city_en: en,
                postal_code: postal,
                area_code: area,
                admin_code: code,
                matched_by: if matched_by.is_empty() {
                    "city"
                } else {
                    matched_by
                },
            });
        }
    }
    if results.is_empty() {
        return Err(PluginError::new("postal.not_found").with("query", args.query.trim()));
    }
    results.truncate(20);
    let total = results.len();
    Ok(Output { results, total })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
