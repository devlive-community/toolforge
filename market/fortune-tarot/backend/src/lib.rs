//! 求签与塔罗插件后端（仅供娱乐与民俗参考）：求签解签、塔罗牌阵与每日运势卡。
//! 随机由日期/调用方提供的种子驱动（splitmix64），同一种子结果完全一致，可复现验证。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct FortuneTarot {
    manifest: Manifest,
}

impl Default for FortuneTarot {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for FortuneTarot {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "stick" => to_value(stick(parse_args(args)?)?),
            "tarot" => to_value(tarot(parse_args(args)?)?),
            "daily" => to_value(daily(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(FortuneTarot);

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    pub fn draw(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.draw() % n as u64) as usize
    }
}

/// 签文库（民间观音灵签风格的自拟文案，非宗教经文）；等级：upper 上 / mid 中 / lower 下
struct Stick {
    level: &'static str,
    title: &'static str,
    text_zh: &'static str,
    text_en: &'static str,
}

const STICKS: &[Stick] = &[
    Stick {
        level: "upper",
        title: "第一签·春风得意",
        text_zh: "东风得意花满蹊，万事顺遂莫迟疑。行舟正值顺风起，一路青云何足齐。",
        text_en: "With spring wind at your back, everything moves forward. Do not hesitate.",
    },
    Stick {
        level: "upper",
        title: "第二签·积善余庆",
        text_zh: "平日耕耘今得果，善缘处处见花开。旧岁辛苦皆成器，新人旧友两相谐。",
        text_en: "Past effort bears fruit; goodwill opens doors in every direction.",
    },
    Stick {
        level: "upper",
        title: "第三签·云开月明",
        text_zh: "云散月明天地宽，前程有路不须愁。心中所愿皆可得，静待时来自在游。",
        text_en: "Clouds part, the moon shines. What you wish for is within reach.",
    },
    Stick {
        level: "mid",
        title: "第四签·守正待时",
        text_zh: "守得云开见月明，莫因小利乱心旌。稳步徐行终有获，急功近利反难成。",
        text_en: "Hold steady and wait. Haste brings little; patience brings reward.",
    },
    Stick {
        level: "mid",
        title: "第五签·平步缓行",
        text_zh: "不徐不疾是坦途，得失从来一念无。愿君平心观世事，风来雨去自如如。",
        text_en: "A steady pace on level ground. Take wins and losses lightly.",
    },
    Stick {
        level: "mid",
        title: "第六签·磨砖作镜",
        text_zh: "此事工多见效迟，功夫到处自逢时。莫嫌琐碎费心力，铁杵磨成绣针枝。",
        text_en: "Slow progress is still progress; keep at it and results come.",
    },
    Stick {
        level: "mid",
        title: "第七签·隔岸观花",
        text_zh: "花在隔岸香扑鼻，欲折还须渡水来。机会当前须主动，空望不前事难谐。",
        text_en: "Opportunity is close but requires action; waiting alone gains nothing.",
    },
    Stick {
        level: "lower",
        title: "第八签·逆水行舟",
        text_zh: "逆水行舟用力撑，一篙松劲退千寻。此时宜守不宜进，养精蓄锐待春风。",
        text_en: "Rowing against the current: hold ground, conserve energy, wait for the tailwind.",
    },
    Stick {
        level: "lower",
        title: "第九签·月被云遮",
        text_zh: "浮云蔽月暂时昏，守己安然待天晴。是非只当风中絮，不必挂怀损精神。",
        text_en: "Temporary gloom passes. Ignore gossip and keep your peace of mind.",
    },
    Stick {
        level: "mid",
        title: "第十签·旱苗得雨",
        text_zh: "旱苗得雨芽频发，枯木逢春再发芽。困顿之时逢转机，此时动静两无差。",
        text_en: "Rain reaches thirsty sprouts: a turn for the better is underway.",
    },
    Stick {
        level: "upper",
        title: "第十一签·金榜题名",
        text_zh: "十年窗下功夫足，一举成名天下知。学海辛勤今有报，从此平步上云梯。",
        text_en: "Long study pays off; recognition and advancement arrive together.",
    },
    Stick {
        level: "mid",
        title: "第十二签·舟行浅滩",
        text_zh: "舟行浅滩须谨慎，载重前行恐搁浅。减负轻装寻深水，稳中求进莫争先。",
        text_en: "Travel light and choose deeper water; overloading strands the boat.",
    },
];

/// 塔罗大阿卡纳（22 张）：名称中英 + 正位/逆位一句话含义
struct Arcana {
    key: &'static str,
    name_zh: &'static str,
    name_en: &'static str,
    upright_zh: &'static str,
    upright_en: &'static str,
    reversed_zh: &'static str,
    reversed_en: &'static str,
}

const ARCANA: &[Arcana] = &[
    Arcana {
        key: "fool",
        name_zh: "愚者",
        name_en: "The Fool",
        upright_zh: "新的开始，保持开放与好奇",
        upright_en: "New beginnings, stay curious",
        reversed_zh: "鲁莽行事，缺乏准备",
        reversed_en: "Recklessness, poor preparation",
    },
    Arcana {
        key: "magician",
        name_zh: "魔术师",
        name_en: "The Magician",
        upright_zh: "行动力与创造力兼备",
        upright_en: "Resourcefulness and willpower",
        reversed_zh: "意图不明，才能被浪费",
        reversed_en: "Untapped talent, unclear intent",
    },
    Arcana {
        key: "priestess",
        name_zh: "女祭司",
        name_en: "The High Priestess",
        upright_zh: "倾听直觉，静观其变",
        upright_en: "Intuition and inner voice",
        reversed_zh: "忽视内心，秘密浮现",
        reversed_en: "Disconnection from intuition",
    },
    Arcana {
        key: "empress",
        name_zh: "女皇",
        name_en: "The Empress",
        upright_zh: "丰饶滋养，成果初现",
        upright_en: "Abundance and nurturing",
        reversed_zh: "过度照料，停滞不前",
        reversed_en: "Creative block, over-reliance",
    },
    Arcana {
        key: "emperor",
        name_zh: "皇帝",
        name_en: "The Emperor",
        upright_zh: "秩序与掌控，稳固推进",
        upright_en: "Structure and authority",
        reversed_zh: "僵化固执，权力滥用",
        reversed_en: "Rigidity, misuse of power",
    },
    Arcana {
        key: "hierophant",
        name_zh: "教皇",
        name_en: "The Hierophant",
        upright_zh: "传统智慧，良师指引",
        upright_en: "Tradition and mentorship",
        reversed_zh: "教条束缚，挑战常规",
        reversed_en: "Dogma, questioning convention",
    },
    Arcana {
        key: "lovers",
        name_zh: "恋人",
        name_en: "The Lovers",
        upright_zh: "重要选择，心意相通",
        upright_en: "Partnership and choice",
        reversed_zh: "分歧失衡，价值冲突",
        reversed_en: "Misalignment, conflict",
    },
    Arcana {
        key: "chariot",
        name_zh: "战车",
        name_en: "The Chariot",
        upright_zh: "意志坚定，乘胜追击",
        upright_en: "Determination and victory",
        reversed_zh: "方向迷失，力量分散",
        reversed_en: "Lost direction, scattered force",
    },
    Arcana {
        key: "strength",
        name_zh: "力量",
        name_en: "Strength",
        upright_zh: "以柔克刚，内在勇气",
        upright_en: "Courage with gentleness",
        reversed_zh: "自我怀疑，情绪失控",
        reversed_en: "Self-doubt, raw emotion",
    },
    Arcana {
        key: "hermit",
        name_zh: "隐士",
        name_en: "The Hermit",
        upright_zh: "独处自省，寻找答案",
        upright_en: "Reflection and guidance",
        reversed_zh: "孤立封闭，逃避现实",
        reversed_en: "Isolation, avoidance",
    },
    Arcana {
        key: "wheel",
        name_zh: "命运之轮",
        name_en: "Wheel of Fortune",
        upright_zh: "周期更替，顺势而为",
        upright_en: "Cycles turning in your favor",
        reversed_zh: "时运不济，抗拒变化",
        reversed_en: "Resistance to change",
    },
    Arcana {
        key: "justice",
        name_zh: "正义",
        name_en: "Justice",
        upright_zh: "因果分明，公正抉择",
        upright_en: "Fairness and accountability",
        reversed_zh: "偏颇不公，逃避责任",
        reversed_en: "Bias, avoiding accountability",
    },
    Arcana {
        key: "hanged",
        name_zh: "倒吊人",
        name_en: "The Hanged Man",
        upright_zh: "换位思考，暂停亦是前行",
        upright_en: "New perspective through pause",
        reversed_zh: "无谓牺牲，停滞不决",
        reversed_en: "Stalling, needless sacrifice",
    },
    Arcana {
        key: "death",
        name_zh: "死神",
        name_en: "Death",
        upright_zh: "告别过去，重生开始",
        upright_en: "Endings that enable rebirth",
        reversed_zh: "抗拒结束，旧事纠缠",
        reversed_en: "Clinging to the past",
    },
    Arcana {
        key: "temperance",
        name_zh: "节制",
        name_en: "Temperance",
        upright_zh: "平衡调和，恰到好处",
        upright_en: "Balance and moderation",
        reversed_zh: "失衡过激，顾此失彼",
        reversed_en: "Imbalance, excess",
    },
    Arcana {
        key: "devil",
        name_zh: "恶魔",
        name_en: "The Devil",
        upright_zh: "直面执念与诱惑",
        upright_en: "Face attachments and temptation",
        reversed_zh: "开始挣脱，断开束缚",
        reversed_en: "Breaking free of chains",
    },
    Arcana {
        key: "tower",
        name_zh: "高塔",
        name_en: "The Tower",
        upright_zh: "旧结构崩塌，真相凸显",
        upright_en: "Sudden change reveals truth",
        reversed_zh: "余震未平，延迟的危机",
        reversed_en: "Averted or delayed collapse",
    },
    Arcana {
        key: "star",
        name_zh: "星星",
        name_en: "The Star",
        upright_zh: "希望重现，静待佳音",
        upright_en: "Hope and renewal",
        reversed_zh: "信心动摇，期望落空",
        reversed_en: "Waning faith",
    },
    Arcana {
        key: "moon",
        name_zh: "月亮",
        name_en: "The Moon",
        upright_zh: "迷雾之中，直觉引路",
        upright_en: "Uncertainty, trust intuition",
        reversed_zh: "迷雾渐散，真相将明",
        reversed_en: "Confusion lifting",
    },
    Arcana {
        key: "sun",
        name_zh: "太阳",
        name_en: "The Sun",
        upright_zh: "光明喜悦，一切明朗",
        upright_en: "Joy and clarity",
        reversed_zh: "光彩暂敛，尚可乐观",
        reversed_en: "Dimmed optimism",
    },
    Arcana {
        key: "judgement",
        name_zh: "审判",
        name_en: "Judgement",
        upright_zh: "复盘觉醒，召唤前行",
        upright_en: "Awakening and reckoning",
        reversed_zh: "回避审视，故步自封",
        reversed_en: "Avoiding self-review",
    },
    Arcana {
        key: "world",
        name_zh: "世界",
        name_en: "The World",
        upright_zh: "圆满完成，新程将启",
        upright_en: "Completion and wholeness",
        reversed_zh: "差一步圆满，收尾未竟",
        reversed_en: "Unfinished closure",
    },
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StickArgs {
    pub seed: u64,
    /// 问事分类（career / love / study），当前仅影响展示，不改变抽签分布
    #[serde(default)]
    pub category: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StickOutput {
    pub index: u64,
    /// levels.<key>：upper / mid / lower
    pub level: &'static str,
    pub title: &'static str,
    pub text_zh: String,
    pub text_en: String,
    pub seed: u64,
}

pub fn stick(args: StickArgs) -> PluginResult<StickOutput> {
    if let Some(category) = &args.category
        && !matches!(category.as_str(), "career" | "love" | "study")
    {
        return Err(PluginError::new("fortune.bad_category").with("category", category.clone()));
    }
    let mut rng = Rng::new(args.seed);
    let index = rng.draw() % STICKS.len() as u64;
    let s = &STICKS[index as usize];
    Ok(StickOutput {
        index: index + 1,
        level: s.level,
        title: s.title,
        text_zh: s.text_zh.into(),
        text_en: s.text_en.into(),
        seed: args.seed,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TarotArgs {
    pub seed: u64,
    /// single（单张）或 three（过去现在未来三张）
    #[serde(default)]
    pub spread: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TarotCard {
    pub key: &'static str,
    pub name_zh: &'static str,
    pub name_en: &'static str,
    /// true = 正位
    pub upright: bool,
    pub meaning_zh: String,
    pub meaning_en: String,
    /// 牌阵位置键（positions.<key>）
    pub position: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TarotOutput {
    pub spread: String,
    pub cards: Vec<TarotCard>,
    pub seed: u64,
}

pub fn tarot(args: TarotArgs) -> PluginResult<TarotOutput> {
    let spread = args.spread.unwrap_or_else(|| "single".into());
    let positions: &[&str] = match spread.as_str() {
        "single" => &["single"],
        "three" => &["past", "present", "future"],
        other => return Err(PluginError::new("fortune.bad_spread").with("spread", other)),
    };
    let mut rng = Rng::new(args.seed);
    let mut used: Vec<usize> = Vec::new();
    let mut cards = Vec::new();
    for position in positions {
        let mut index = rng.below(ARCANA.len());
        while used.contains(&index) {
            index = (index + 1) % ARCANA.len();
        }
        used.push(index);
        let arcana = &ARCANA[index];
        let upright = rng.draw().is_multiple_of(2);
        cards.push(TarotCard {
            key: arcana.key,
            name_zh: arcana.name_zh,
            name_en: arcana.name_en,
            upright,
            meaning_zh: if upright {
                arcana.upright_zh.into()
            } else {
                arcana.reversed_zh.into()
            },
            meaning_en: if upright {
                arcana.upright_en.into()
            } else {
                arcana.reversed_en.into()
            },
            position,
        });
    }
    Ok(TarotOutput {
        spread,
        cards,
        seed: args.seed,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyArgs {
    /// 日期数字（如 20261103），同一天结果一致
    pub date: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyOutput {
    pub date: u64,
    /// fortune 卡：一张大阿卡纳 + 等级
    pub card: TarotCard,
    /// levels.<key>
    pub level: &'static str,
    pub seed: u64,
}

pub fn daily(args: DailyArgs) -> PluginResult<DailyOutput> {
    if args.date < 19700101 || args.date > 99991231 {
        return Err(PluginError::new("fortune.bad_date").with("date", args.date));
    }
    let mut rng = Rng::new(args.date.wrapping_mul(0x517C_C1B7_2722_0A95));
    let index = rng.below(ARCANA.len());
    let arcana = &ARCANA[index];
    let upright = rng.draw().is_multiple_of(2);
    let level = match (index + (upright as usize)) % 3 {
        0 => "upper",
        1 => "mid",
        _ => "lower",
    };
    Ok(DailyOutput {
        date: args.date,
        level,
        seed: args.date,
        card: TarotCard {
            key: arcana.key,
            name_zh: arcana.name_zh,
            name_en: arcana.name_en,
            upright,
            meaning_zh: if upright {
                arcana.upright_zh.into()
            } else {
                arcana.reversed_zh.into()
            },
            meaning_en: if upright {
                arcana.upright_en.into()
            } else {
                arcana.reversed_en.into()
            },
            position: "daily",
        },
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
