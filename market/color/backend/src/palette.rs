//! 中国传统色卡：广为流传的传统色名与色值（公有领域的文化数据），
//! 按色系分组，供界面浏览与 `palette` 查询。

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Swatch {
    /// 中文色名（原样返回；界面用 locales 的 `palette.names.<hex>` 提供英文名）
    pub name: &'static str,
    pub pinyin: &'static str,
    pub hex: &'static str,
    /// CATEGORIES 之一
    pub category: &'static str,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    /// 按色名、拼音或色值过滤（大小写不敏感）；空为全部
    #[serde(default)]
    pub query: String,
}

#[derive(Debug, Serialize)]
pub struct Output {
    /// 命中的色系（保持 CATEGORIES 顺序，只含有命中的分组）
    pub groups: Vec<Group>,
    pub total: usize,
}

/// 静态色表条目；输出结构 [`Group`] 的 colors 是按查询过滤后的动态列表
struct StaticGroup {
    category: &'static str,
    colors: &'static [Swatch],
}

#[derive(Debug, Serialize)]
pub struct Group {
    pub category: &'static str,
    pub colors: Vec<Swatch>,
}

static GROUPS: &[StaticGroup] = &[
    StaticGroup {
        category: "red",
        colors: &[
            Swatch {
                name: "朱砂",
                pinyin: "zhusha",
                hex: "#ff461f",
                category: "red",
            },
            Swatch {
                name: "赤",
                pinyin: "chi",
                hex: "#c3272b",
                category: "red",
            },
            Swatch {
                name: "胭脂",
                pinyin: "yanzhi",
                hex: "#9d2933",
                category: "red",
            },
            Swatch {
                name: "石榴红",
                pinyin: "shilihong",
                hex: "#f20c00",
                category: "red",
            },
            Swatch {
                name: "樱桃红",
                pinyin: "yingtaohong",
                hex: "#ed5126",
                category: "red",
            },
            Swatch {
                name: "豇豆红",
                pinyin: "jiangdouhong",
                hex: "#bc4f43",
                category: "red",
            },
            Swatch {
                name: "粉红",
                pinyin: "fenhong",
                hex: "#f7a7ab",
                category: "red",
            },
            Swatch {
                name: "海棠红",
                pinyin: "haitanghong",
                hex: "#db5a6b",
                category: "red",
            },
            Swatch {
                name: "银红",
                pinyin: "yinhong",
                hex: "#f05654",
                category: "red",
            },
            Swatch {
                name: "绛紫",
                pinyin: "jiangzi",
                hex: "#8c4356",
                category: "red",
            },
        ],
    },
    StaticGroup {
        category: "yellow",
        colors: &[
            Swatch {
                name: "藤黄",
                pinyin: "tenghuang",
                hex: "#ffb61e",
                category: "yellow",
            },
            Swatch {
                name: "栀子",
                pinyin: "zhizi",
                hex: "#f2be45",
                category: "yellow",
            },
            Swatch {
                name: "鹅黄",
                pinyin: "ehuang",
                hex: "#fff143",
                category: "yellow",
            },
            Swatch {
                name: "缃色",
                pinyin: "xiangse",
                hex: "#f0c239",
                category: "yellow",
            },
            Swatch {
                name: "杏黄",
                pinyin: "xinghuang",
                hex: "#ffa631",
                category: "yellow",
            },
            Swatch {
                name: "姜黄",
                pinyin: "jianghuang",
                hex: "#e9a23b",
                category: "yellow",
            },
            Swatch {
                name: "秋香色",
                pinyin: "qiuxiangse",
                hex: "#d9b611",
                category: "yellow",
            },
            Swatch {
                name: "琥珀",
                pinyin: "hupo",
                hex: "#ca6924",
                category: "yellow",
            },
            Swatch {
                name: "明黄",
                pinyin: "minghuang",
                hex: "#f9bd08",
                category: "yellow",
            },
            Swatch {
                name: "驼",
                pinyin: "tuo",
                hex: "#a88462",
                category: "yellow",
            },
        ],
    },
    StaticGroup {
        category: "green",
        colors: &[
            Swatch {
                name: "葱倩",
                pinyin: "congqian",
                hex: "#9ed048",
                category: "green",
            },
            Swatch {
                name: "豆绿",
                pinyin: "doulv",
                hex: "#9ed048",
                category: "green",
            },
            Swatch {
                name: "松花",
                pinyin: "songhua",
                hex: "#bce672",
                category: "green",
            },
            Swatch {
                name: "柳绿",
                pinyin: "liulv",
                hex: "#51b445",
                category: "green",
            },
            Swatch {
                name: "葱青",
                pinyin: "congqing",
                hex: "#0eb83a",
                category: "green",
            },
            Swatch {
                name: "油绿",
                pinyin: "youlv",
                hex: "#005831",
                category: "green",
            },
            Swatch {
                name: "翡翠",
                pinyin: "feicui",
                hex: "#3ced3c",
                category: "green",
            },
            Swatch {
                name: "竹青",
                pinyin: "zhuqing",
                hex: "#789262",
                category: "green",
            },
            Swatch {
                name: "石绿",
                pinyin: "shilv",
                hex: "#16a951",
                category: "green",
            },
            Swatch {
                name: "松柏绿",
                pinyin: "songbailv",
                hex: "#21a675",
                category: "green",
            },
        ],
    },
    StaticGroup {
        category: "cyan",
        colors: &[
            Swatch {
                name: "天缥",
                pinyin: "tianpiao",
                hex: "#d2f0f4",
                category: "cyan",
            },
            Swatch {
                name: "碧色",
                pinyin: "bise",
                hex: "#1bd1a5",
                category: "cyan",
            },
            Swatch {
                name: "石青",
                pinyin: "shiqing",
                hex: "#1685a9",
                category: "cyan",
            },
            Swatch {
                name: "靛青",
                pinyin: "dianqing",
                hex: "#177cb0",
                category: "cyan",
            },
            Swatch {
                name: "孔雀蓝",
                pinyin: "kongquelan",
                hex: "#33a1c9",
                category: "cyan",
            },
            Swatch {
                name: "湖绿",
                pinyin: "hulv",
                hex: "#30d5c8",
                category: "cyan",
            },
            Swatch {
                name: "青白",
                pinyin: "qingbai",
                hex: "#c0ebd7",
                category: "cyan",
            },
            Swatch {
                name: "蟹壳青",
                pinyin: "xiekeqing",
                hex: "#02252a",
                category: "cyan",
            },
            Swatch {
                name: "鸭卵青",
                pinyin: "yaluanqing",
                hex: "#e0eee8",
                category: "cyan",
            },
            Swatch {
                name: "苍色",
                pinyin: "cangse",
                hex: "#75878a",
                category: "cyan",
            },
        ],
    },
    StaticGroup {
        category: "blue",
        colors: &[
            Swatch {
                name: "霁蓝",
                pinyin: "jilan",
                hex: "#0f1423",
                category: "blue",
            },
            Swatch {
                name: "群青",
                pinyin: "qunqing",
                hex: "#4c8dae",
                category: "blue",
            },
            Swatch {
                name: "藏蓝",
                pinyin: "zanglan",
                hex: "#3b2e7e",
                category: "blue",
            },
            Swatch {
                name: "宝蓝",
                pinyin: "baolan",
                hex: "#4b5cc4",
                category: "blue",
            },
            Swatch {
                name: "黛蓝",
                pinyin: "dailan",
                hex: "#425066",
                category: "blue",
            },
            Swatch {
                name: "绀青",
                pinyin: "ganqing",
                hex: "#003371",
                category: "blue",
            },
            Swatch {
                name: "蔚蓝",
                pinyin: "weilan",
                hex: "#2e59a7",
                category: "blue",
            },
            Swatch {
                name: "靛蓝",
                pinyin: "dianlan",
                hex: "#065279",
                category: "blue",
            },
            Swatch {
                name: "湛蓝",
                pinyin: "zhanlan",
                hex: "#3aafda",
                category: "blue",
            },
            Swatch {
                name: "月白",
                pinyin: "yuebai",
                hex: "#d6ecf0",
                category: "blue",
            },
        ],
    },
    StaticGroup {
        category: "purple",
        colors: &[
            Swatch {
                name: "紫棠",
                pinyin: "zitang",
                hex: "#56004f",
                category: "purple",
            },
            Swatch {
                name: "青莲",
                pinyin: "qinglian",
                hex: "#801dae",
                category: "purple",
            },
            Swatch {
                name: "紫藤",
                pinyin: "ziteng",
                hex: "#a29ac3",
                category: "purple",
            },
            Swatch {
                name: "黛紫",
                pinyin: "daizi",
                hex: "#574266",
                category: "purple",
            },
            Swatch {
                name: "藕色",
                pinyin: "ouse",
                hex: "#edd1d8",
                category: "purple",
            },
            Swatch {
                name: "葡萄紫",
                pinyin: "putaozi",
                hex: "#4b2e2b",
                category: "purple",
            },
            Swatch {
                name: "丁香",
                pinyin: "dingxiang",
                hex: "#cca4e3",
                category: "purple",
            },
            Swatch {
                name: "雪青",
                pinyin: "xueqing",
                hex: "#b0a4e3",
                category: "purple",
            },
            Swatch {
                name: "绀紫",
                pinyin: "ganzi",
                hex: "#815476",
                category: "purple",
            },
            Swatch {
                name: "紫酱",
                pinyin: "zijiang",
                hex: "#815c74",
                category: "purple",
            },
        ],
    },
    StaticGroup {
        category: "brown",
        colors: &[
            Swatch {
                name: "棕红",
                pinyin: "zonghong",
                hex: "#9b4400",
                category: "brown",
            },
            Swatch {
                name: "赭",
                pinyin: "zhe",
                hex: "#955539",
                category: "brown",
            },
            Swatch {
                name: "驼色",
                pinyin: "tuose",
                hex: "#8c6d50",
                category: "brown",
            },
            Swatch {
                name: "栗色",
                pinyin: "lise",
                hex: "#60281e",
                category: "brown",
            },
            Swatch {
                name: "酱色",
                pinyin: "jiangse",
                hex: "#6b2f1c",
                category: "brown",
            },
            Swatch {
                name: "咖啡",
                pinyin: "kafei",
                hex: "#6f4c2b",
                category: "brown",
            },
            Swatch {
                name: "茶色",
                pinyin: "chase",
                hex: "#b35c44",
                category: "brown",
            },
            Swatch {
                name: "枣红",
                pinyin: "zaohong",
                hex: "#8c1f28",
                category: "brown",
            },
            Swatch {
                name: "棕黑",
                pinyin: "zonghei",
                hex: "#7c4b00",
                category: "brown",
            },
            Swatch {
                name: "棕色",
                pinyin: "zongse",
                hex: "#6b4226",
                category: "brown",
            },
        ],
    },
    StaticGroup {
        category: "neutral",
        colors: &[
            Swatch {
                name: "月灰",
                pinyin: "yuehui",
                hex: "#d0d0d0",
                category: "neutral",
            },
            Swatch {
                name: "象牙白",
                pinyin: "xiangyabai",
                hex: "#fffbf0",
                category: "neutral",
            },
            Swatch {
                name: "缟",
                pinyin: "gao",
                hex: "#f2ecde",
                category: "neutral",
            },
            Swatch {
                name: "霜色",
                pinyin: "shuangse",
                hex: "#e9f1f6",
                category: "neutral",
            },
            Swatch {
                name: "漆黑",
                pinyin: "qihei",
                hex: "#161823",
                category: "neutral",
            },
            Swatch {
                name: "乌黑",
                pinyin: "wuhei",
                hex: "#392f41",
                category: "neutral",
            },
            Swatch {
                name: "玄青",
                pinyin: "xuanqing",
                hex: "#3d3b4f",
                category: "neutral",
            },
            Swatch {
                name: "墨色",
                pinyin: "mose",
                hex: "#50616d",
                category: "neutral",
            },
            Swatch {
                name: "黔",
                pinyin: "qian",
                hex: "#445766",
                category: "neutral",
            },
            Swatch {
                name: "铅白",
                pinyin: "qianbai",
                hex: "#f0f0f4",
                category: "neutral",
            },
        ],
    },
];

fn matches(swatch: &Swatch, needle: &str) -> bool {
    swatch.name.contains(needle)
        || swatch.pinyin.contains(needle)
        || swatch.hex.to_lowercase().contains(needle)
}

pub fn palette(args: Args) -> PluginResult<Output> {
    let needle = args.query.trim().to_lowercase();
    let mut groups = Vec::new();
    let mut total = 0;
    for group in GROUPS {
        let colors: Vec<Swatch> = if needle.is_empty() {
            group.colors.to_vec()
        } else {
            group
                .colors
                .iter()
                .filter(|swatch| matches(swatch, &needle))
                .copied()
                .collect()
        };
        if colors.is_empty() {
            continue;
        }
        total += colors.len();
        groups.push(Group {
            category: group.category,
            colors,
        });
    }
    if groups.is_empty() && !needle.is_empty() {
        return Err(PluginError::new("color.not_found").with("query", args.query.trim()));
    }
    Ok(Output { groups, total })
}
