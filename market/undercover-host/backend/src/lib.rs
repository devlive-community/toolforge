//! 谁是卧底主持台后端：随机发词、回合状态机（描述→投票→平票重投→胜负）与
//! 对局摘要。角色分配与计票完全由 Rust 处理；前端只上报玩家动作。
//! 当前局状态由前端持有并整体回传（无服务器会话），词对库内置自创词对。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct UndercoverHost {
    manifest: Manifest,
}

impl Default for UndercoverHost {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for UndercoverHost {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "deal" => to_value(deal(parse_args(args)?)?),
            "tally" => to_value(tally(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(UndercoverHost);

/// 内置自创词对（平民词, 卧底词）
pub const WORD_PAIRS: &[(&str, &str, &str)] = &[
    ("food", "火锅", "麻辣烫"),
    ("food", "汉堡", "肉夹馍"),
    ("food", "饺子", "馄饨"),
    ("life", "牙刷", "梳子"),
    ("life", "雨伞", "遮阳伞"),
    ("life", "拖鞋", "凉鞋"),
    ("place", "图书馆", "书店"),
    ("place", "游乐园", "动物园"),
    ("place", "地铁站", "火车站"),
    ("work", "会议", "培训"),
    ("work", "周报", "月报"),
    ("tech", "手机", "平板"),
    ("tech", "耳机", "音箱"),
    ("sport", "跑步", "竞走"),
    ("sport", "篮球", "排球"),
];

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    pub fn below(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) % n as u64) as usize
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DealArgs {
    /// 玩家昵称（4–12 个，去重后编号）
    pub players: Vec<String>,
    /// 词对类别；缺省随机
    #[serde(default)]
    pub category: Option<String>,
    /// 自定义词对（覆盖内置库）
    #[serde(default)]
    pub custom_pair: Option<(String, String)>,
    pub seed: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerWord {
    pub seat: usize,
    pub name: String,
    /// civilian / undercover
    pub role: String,
    pub word: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DealOutput {
    pub players: Vec<PlayerWord>,
    /// 类别（内置库用；自定义时为 custom）
    pub category: String,
    /// 本局词对（揭晓用）
    pub pair: (String, String),
}

/// 发词：随机选卧底与词对；重名自动加编号
pub fn deal(args: DealArgs) -> PluginResult<DealOutput> {
    let n = args.players.len();
    if !(4..=12).contains(&n) {
        return Err(PluginError::new("undercover.bad_player_count").with("count", n as u64));
    }
    let mut names: Vec<String> = Vec::new();
    for name in &args.players {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(PluginError::new("undercover.empty_name"));
        }
        // 重名编号：张三、张三(2)
        let base = trimmed.to_owned();
        let mut final_name = base.clone();
        let mut suffix = 2;
        while names.contains(&final_name) {
            final_name = format!("{base}({suffix})");
            suffix += 1;
        }
        names.push(final_name);
    }
    let (category, pair) = match (&args.custom_pair, args.category.as_deref()) {
        (Some((civ, und)), _) => ("custom".to_owned(), (civ.clone(), und.clone())),
        (None, Some(category)) => {
            let found = WORD_PAIRS
                .iter()
                .find(|(c, _, _)| *c == category)
                .ok_or_else(|| {
                    PluginError::new("undercover.bad_category").with("category", category)
                })?;
            (found.0.to_owned(), (found.1.to_owned(), found.2.to_owned()))
        }
        (None, None) => {
            let mut rng = Rng::new(args.seed);
            let found = WORD_PAIRS[rng.below(WORD_PAIRS.len())];
            (found.0.to_owned(), (found.1.to_owned(), found.2.to_owned()))
        }
    };
    if pair.0 == pair.1 {
        return Err(PluginError::new("undercover.same_words"));
    }
    let mut rng = Rng::new(args.seed);
    let undercover_seat = rng.below(n);
    let players = names
        .iter()
        .enumerate()
        .map(|(seat, name)| {
            let (role, word) = if seat == undercover_seat {
                ("undercover", pair.1.clone())
            } else {
                ("civilian", pair.0.clone())
            };
            PlayerWord {
                seat,
                name: name.clone(),
                role: role.into(),
                word,
            }
        })
        .collect();
    Ok(DealOutput {
        players,
        category,
        pair,
    })
}

/// 一轮投票：多数出局；平票时返回并列候选（前端重投一次），再次平票无人出局
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TallyArgs {
    /// 本轮可投票的玩家座位（存活者）
    pub alive: Vec<usize>,
    /// 每位存活者的投票（座位号；null 弃票）
    pub votes: Vec<Option<usize>>,
    /// 是否重投轮（重投仅在并列候选间计票，仍平票则无人出局）
    #[serde(default)]
    pub runoff: bool,
    /// 重投候选座位（runoff = true 时必填）
    #[serde(default)]
    pub runoff_candidates: Vec<usize>,
    /// 卧底座位（胜负判定）
    pub undercover: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TallyOutput {
    /// eliminated（有人出局）/ tie（平票，附 runoff_candidates）/ runoff_stalemate（重投仍平）/ victory_civilian / victory_undercover
    pub status: &'static str,
    /// 出局者座位（eliminated 时）
    pub eliminated: Option<usize>,
    /// 平票候选（tie 时）
    pub runoff_candidates: Vec<usize>,
    /// 各候选得票（展示用）
    pub counts: Vec<Count>,
    /// 存活座位（出局后）
    pub alive: Vec<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Count {
    pub seat: usize,
    pub votes: usize,
}

pub fn tally(args: TallyArgs) -> PluginResult<TallyOutput> {
    if args.alive.is_empty() {
        return Err(PluginError::new("undercover.no_alive"));
    }
    // 投票合法性：投票者必须存活；目标必须在本轮允许范围内且非自投
    let mut counts: Vec<(usize, usize)> = Vec::new();
    let targets: &[usize] = if args.runoff {
        &args.runoff_candidates
    } else {
        &args.alive
    };
    for (index, vote) in args.votes.iter().enumerate() {
        let voter = *args
            .alive
            .get(index)
            .ok_or_else(|| PluginError::new("undercover.voter_mismatch"))?;
        if let Some(target) = vote {
            if !targets.contains(target) {
                return Err(PluginError::new("undercover.bad_target").with("seat", *target as u64));
            }
            if !args.runoff && *target == voter {
                return Err(PluginError::new("undercover.self_vote").with("seat", voter as u64));
            }
            match counts.iter_mut().find(|(seat, _)| seat == target) {
                Some((_, count)) => *count += 1,
                None => counts.push((*target, 1)),
            }
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let max = counts.first().map(|(_, c)| *c).unwrap_or(0);
    let top: Vec<usize> = counts
        .iter()
        .filter(|(_, c)| *c == max && *c > 0)
        .map(|(s, _)| *s)
        .collect();
    let mut alive_after = args.alive.clone();
    let status;
    let eliminated;
    let runoff_candidates;
    if top.is_empty() {
        // 全员弃票：无人出局
        status = "runoff_stalemate";
        eliminated = None;
        runoff_candidates = Vec::new();
    } else if top.len() == 1 {
        status = "eliminated";
        eliminated = Some(top[0]);
        runoff_candidates = Vec::new();
        alive_after.retain(|seat| *seat != top[0]);
    } else if args.runoff {
        // 重投仍平票：本轮无人出局
        status = "runoff_stalemate";
        eliminated = None;
        runoff_candidates = Vec::new();
    } else {
        status = "tie";
        eliminated = None;
        runoff_candidates = top.clone();
    }
    // 胜负：卧底出局 → 平民胜；存活降到 3 且卧底仍在 → 卧底胜
    let status = if status == "eliminated" {
        if eliminated == Some(args.undercover) {
            "victory_civilian"
        } else if alive_after.len() <= 3 {
            "victory_undercover"
        } else {
            status
        }
    } else {
        status
    };
    let count_rows = counts
        .into_iter()
        .map(|(seat, votes)| Count { seat, votes })
        .collect();
    Ok(TallyOutput {
        status,
        eliminated,
        runoff_candidates,
        counts: count_rows,
        alive: alive_after,
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
