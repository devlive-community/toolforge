//! 世界地理速查插件后端：国家概要数据（中英文名、首都、大洲、货币、区号、
//! 人口概数）。数据为公共事实，中英文名直接存于数据行；界面标签走 locales。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct WorldGeography {
    manifest: Manifest,
}

impl Default for WorldGeography {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for WorldGeography {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "list" => to_value(list(parse_args(args)?)?),
            "by_capital" => to_value(by_capital(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(WorldGeography);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Country {
    /// ISO 3166-1 alpha-2
    pub code: &'static str,
    pub name_zh: &'static str,
    pub name_en: &'static str,
    pub capital_zh: &'static str,
    pub capital_en: &'static str,
    /// continents.<key>
    pub continent: &'static str,
    pub currency: &'static str,
    pub calling_code: &'static str,
    /// 人口概数（百万，2024 前后公开统计的约数）
    pub population_m: f64,
}

macro_rules! countries {
    ($([$code:literal, $zh:literal, $en:literal, $cap_zh:literal, $cap_en:literal, $cont:literal, $cur:literal, $tel:literal, $pop:expr]),+ $(,)?) => {
        &[ $( Country {
            code: $code, name_zh: $zh, name_en: $en, capital_zh: $cap_zh, capital_en: $cap_en,
            continent: $cont, currency: $cur, calling_code: $tel, population_m: $pop,
        } ),+ ]
    };
}

static COUNTRIES: &[Country] = countries![
    [
        "CN", "中国", "China", "北京", "Beijing", "asia", "CNY", "+86", 1410.0
    ],
    [
        "JP", "日本", "Japan", "东京", "Tokyo", "asia", "JPY", "+81", 124.0
    ],
    [
        "KR",
        "韩国",
        "South Korea",
        "首尔",
        "Seoul",
        "asia",
        "KRW",
        "+82",
        51.7
    ],
    [
        "KP",
        "朝鲜",
        "North Korea",
        "平壤",
        "Pyongyang",
        "asia",
        "KPW",
        "+850",
        26.0
    ],
    [
        "IN",
        "印度",
        "India",
        "新德里",
        "New Delhi",
        "asia",
        "INR",
        "+91",
        1428.0
    ],
    [
        "ID",
        "印度尼西亚",
        "Indonesia",
        "雅加达",
        "Jakarta",
        "asia",
        "IDR",
        "+62",
        277.0
    ],
    [
        "TH", "泰国", "Thailand", "曼谷", "Bangkok", "asia", "THB", "+66", 71.8
    ],
    [
        "VN", "越南", "Vietnam", "河内", "Hanoi", "asia", "VND", "+84", 98.9
    ],
    [
        "SG",
        "新加坡",
        "Singapore",
        "新加坡",
        "Singapore",
        "asia",
        "SGD",
        "+65",
        5.9
    ],
    [
        "MY",
        "马来西亚",
        "Malaysia",
        "吉隆坡",
        "Kuala Lumpur",
        "asia",
        "MYR",
        "+60",
        34.3
    ],
    [
        "PH",
        "菲律宾",
        "Philippines",
        "马尼拉",
        "Manila",
        "asia",
        "PHP",
        "+63",
        117.3
    ],
    [
        "MM",
        "缅甸",
        "Myanmar",
        "内比都",
        "Naypyidaw",
        "asia",
        "MMK",
        "+95",
        54.6
    ],
    [
        "KH",
        "柬埔寨",
        "Cambodia",
        "金边",
        "Phnom Penh",
        "asia",
        "KHR",
        "+855",
        16.9
    ],
    [
        "LK",
        "斯里兰卡",
        "Sri Lanka",
        "科伦坡",
        "Colombo",
        "asia",
        "LKR",
        "+94",
        22.2
    ],
    [
        "NP",
        "尼泊尔",
        "Nepal",
        "加德满都",
        "Kathmandu",
        "asia",
        "NPR",
        "+977",
        30.9
    ],
    [
        "PK",
        "巴基斯坦",
        "Pakistan",
        "伊斯兰堡",
        "Islamabad",
        "asia",
        "PKR",
        "+92",
        240.5
    ],
    [
        "BD",
        "孟加拉国",
        "Bangladesh",
        "达卡",
        "Dhaka",
        "asia",
        "BDT",
        "+880",
        173.0
    ],
    [
        "KZ",
        "哈萨克斯坦",
        "Kazakhstan",
        "阿斯塔纳",
        "Astana",
        "asia",
        "KZT",
        "+7",
        19.6
    ],
    [
        "SA",
        "沙特阿拉伯",
        "Saudi Arabia",
        "利雅得",
        "Riyadh",
        "asia",
        "SAR",
        "+966",
        36.9
    ],
    [
        "AE",
        "阿联酋",
        "United Arab Emirates",
        "阿布扎比",
        "Abu Dhabi",
        "asia",
        "AED",
        "+971",
        9.5
    ],
    [
        "IL",
        "以色列",
        "Israel",
        "耶路撒冷",
        "Jerusalem",
        "asia",
        "ILS",
        "+972",
        9.8
    ],
    [
        "TR",
        "土耳其",
        "Türkiye",
        "安卡拉",
        "Ankara",
        "asia",
        "TRY",
        "+90",
        85.3
    ],
    [
        "RU",
        "俄罗斯",
        "Russia",
        "莫斯科",
        "Moscow",
        "europe",
        "RUB",
        "+7",
        144.4
    ],
    [
        "DE", "德国", "Germany", "柏林", "Berlin", "europe", "EUR", "+49", 84.5
    ],
    [
        "FR", "法国", "France", "巴黎", "Paris", "europe", "EUR", "+33", 68.2
    ],
    [
        "GB",
        "英国",
        "United Kingdom",
        "伦敦",
        "London",
        "europe",
        "GBP",
        "+44",
        68.4
    ],
    [
        "IT",
        "意大利",
        "Italy",
        "罗马",
        "Rome",
        "europe",
        "EUR",
        "+39",
        58.9
    ],
    [
        "ES",
        "西班牙",
        "Spain",
        "马德里",
        "Madrid",
        "europe",
        "EUR",
        "+34",
        48.4
    ],
    [
        "PT",
        "葡萄牙",
        "Portugal",
        "里斯本",
        "Lisbon",
        "europe",
        "EUR",
        "+351",
        10.5
    ],
    [
        "NL",
        "荷兰",
        "Netherlands",
        "阿姆斯特丹",
        "Amsterdam",
        "europe",
        "EUR",
        "+31",
        17.9
    ],
    [
        "BE",
        "比利时",
        "Belgium",
        "布鲁塞尔",
        "Brussels",
        "europe",
        "EUR",
        "+32",
        11.8
    ],
    [
        "CH",
        "瑞士",
        "Switzerland",
        "伯尔尼",
        "Bern",
        "europe",
        "CHF",
        "+41",
        8.8
    ],
    [
        "AT",
        "奥地利",
        "Austria",
        "维也纳",
        "Vienna",
        "europe",
        "EUR",
        "+43",
        9.1
    ],
    [
        "SE",
        "瑞典",
        "Sweden",
        "斯德哥尔摩",
        "Stockholm",
        "europe",
        "SEK",
        "+46",
        10.5
    ],
    [
        "NO",
        "挪威",
        "Norway",
        "奥斯陆",
        "Oslo",
        "europe",
        "NOK",
        "+47",
        5.5
    ],
    [
        "DK",
        "丹麦",
        "Denmark",
        "哥本哈根",
        "Copenhagen",
        "europe",
        "DKK",
        "+45",
        5.9
    ],
    [
        "FI",
        "芬兰",
        "Finland",
        "赫尔辛基",
        "Helsinki",
        "europe",
        "EUR",
        "+358",
        5.6
    ],
    [
        "PL", "波兰", "Poland", "华沙", "Warsaw", "europe", "PLN", "+48", 36.8
    ],
    [
        "CZ",
        "捷克",
        "Czechia",
        "布拉格",
        "Prague",
        "europe",
        "CZK",
        "+420",
        10.9
    ],
    [
        "GR", "希腊", "Greece", "雅典", "Athens", "europe", "EUR", "+30", 10.4
    ],
    [
        "UA",
        "乌克兰",
        "Ukraine",
        "基辅",
        "Kyiv",
        "europe",
        "UAH",
        "+380",
        36.7
    ],
    [
        "IE",
        "爱尔兰",
        "Ireland",
        "都柏林",
        "Dublin",
        "europe",
        "EUR",
        "+353",
        5.3
    ],
    [
        "HU",
        "匈牙利",
        "Hungary",
        "布达佩斯",
        "Budapest",
        "europe",
        "HUF",
        "+36",
        9.6
    ],
    [
        "US",
        "美国",
        "United States",
        "华盛顿哥伦比亚特区",
        "Washington, D.C.",
        "north-america",
        "USD",
        "+1",
        335.0
    ],
    [
        "CA",
        "加拿大",
        "Canada",
        "渥太华",
        "Ottawa",
        "north-america",
        "CAD",
        "+1",
        40.1
    ],
    [
        "MX",
        "墨西哥",
        "Mexico",
        "墨西哥城",
        "Mexico City",
        "north-america",
        "MXN",
        "+52",
        128.5
    ],
    [
        "CU",
        "古巴",
        "Cuba",
        "哈瓦那",
        "Havana",
        "north-america",
        "CUP",
        "+53",
        11.2
    ],
    [
        "BR",
        "巴西",
        "Brazil",
        "巴西利亚",
        "Brasília",
        "south-america",
        "BRL",
        "+55",
        216.4
    ],
    [
        "AR",
        "阿根廷",
        "Argentina",
        "布宜诺斯艾利斯",
        "Buenos Aires",
        "south-america",
        "ARS",
        "+54",
        45.8
    ],
    [
        "CL",
        "智利",
        "Chile",
        "圣地亚哥",
        "Santiago",
        "south-america",
        "CLP",
        "+56",
        19.6
    ],
    [
        "CO",
        "哥伦比亚",
        "Colombia",
        "波哥大",
        "Bogotá",
        "south-america",
        "COP",
        "+57",
        52.1
    ],
    [
        "PE",
        "秘鲁",
        "Peru",
        "利马",
        "Lima",
        "south-america",
        "PEN",
        "+51",
        34.4
    ],
    [
        "EG", "埃及", "Egypt", "开罗", "Cairo", "africa", "EGP", "+20", 112.7
    ],
    [
        "ZA",
        "南非",
        "South Africa",
        "比勒陀利亚",
        "Pretoria",
        "africa",
        "ZAR",
        "+27",
        60.4
    ],
    [
        "NG",
        "尼日利亚",
        "Nigeria",
        "阿布贾",
        "Abuja",
        "africa",
        "NGN",
        "+234",
        223.8
    ],
    [
        "KE",
        "肯尼亚",
        "Kenya",
        "内罗毕",
        "Nairobi",
        "africa",
        "KES",
        "+254",
        55.1
    ],
    [
        "ET",
        "埃塞俄比亚",
        "Ethiopia",
        "亚的斯亚贝巴",
        "Addis Ababa",
        "africa",
        "ETB",
        "+251",
        126.5
    ],
    [
        "MA",
        "摩洛哥",
        "Morocco",
        "拉巴特",
        "Rabat",
        "africa",
        "MAD",
        "+212",
        37.8
    ],
    [
        "GH",
        "加纳",
        "Ghana",
        "阿克拉",
        "Accra",
        "africa",
        "GHS",
        "+233",
        34.1
    ],
    [
        "TZ",
        "坦桑尼亚",
        "Tanzania",
        "多多马",
        "Dodoma",
        "africa",
        "TZS",
        "+255",
        67.4
    ],
    [
        "AU",
        "澳大利亚",
        "Australia",
        "堪培拉",
        "Canberra",
        "oceania",
        "AUD",
        "+61",
        26.6
    ],
    [
        "NZ",
        "新西兰",
        "New Zealand",
        "惠灵顿",
        "Wellington",
        "oceania",
        "NZD",
        "+64",
        5.2
    ],
    [
        "FJ", "斐济", "Fiji", "苏瓦", "Suva", "oceania", "FJD", "+679", 0.9
    ],
];

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListArgs {
    /// 匹配中英文名、首都、代码或区号
    #[serde(default)]
    pub query: String,
    /// continents.<key>
    #[serde(default)]
    pub continent: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListOutput {
    pub countries: Vec<Country>,
    pub total: usize,
}

fn matches(country: &Country, needle: &str) -> bool {
    country.code.eq_ignore_ascii_case(needle)
        || country.name_zh.contains(needle)
        || country.name_en.to_lowercase().contains(needle)
        || country.capital_zh.contains(needle)
        || country.capital_en.to_lowercase().contains(needle)
        || country.calling_code.contains(needle)
}

pub fn list(args: ListArgs) -> PluginResult<ListOutput> {
    let needle = args.query.trim().to_lowercase();
    let countries: Vec<Country> = COUNTRIES
        .iter()
        .filter(|c| args.continent.as_ref().is_none_or(|k| c.continent == k))
        .filter(|c| needle.is_empty() || matches(c, &needle))
        .cloned()
        .collect();
    if countries.is_empty() && !needle.is_empty() {
        return Err(PluginError::new("geo.not_found").with("query", args.query.trim()));
    }
    let total = countries.len();
    Ok(ListOutput { countries, total })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapitalArgs {
    /// 首都名（中英文均可）
    pub capital: String,
}

pub fn by_capital(args: CapitalArgs) -> PluginResult<ListOutput> {
    let needle = args.capital.trim().to_lowercase();
    let countries: Vec<Country> = COUNTRIES
        .iter()
        .filter(|c| c.capital_zh.contains(&needle) || c.capital_en.to_lowercase().contains(&needle))
        .cloned()
        .collect();
    if countries.is_empty() {
        return Err(PluginError::new("geo.capital_not_found").with("capital", args.capital.trim()));
    }
    let total = countries.len();
    Ok(ListOutput { countries, total })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
