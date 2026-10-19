//! 调度核心：公开透明的固定档位——首次记住 1 天，连续记住 3/7/14/30 天封顶，
//! 没记住回到 1 天档并清零连续计数。日期一律用日历日（本地口径由前端传入），
//! 跨月/闰日交给 jiff 的 civil 日期运算；不虚构逾期期间的复习。

use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

/// 档位间隔（天）：连续第 1..=5 次记住依次使用，之后保持 30 天
pub const STAGES: [i64; 5] = [1, 3, 7, 14, 30];
/// 卡片与牌组上限
pub const MAX_CARDS: usize = 5000;
pub const MAX_DECKS: usize = 64;
pub const MAX_TAGS: usize = 8;
pub const MAX_TEXT: usize = 5000;
pub const MAX_CSV_ROWS: usize = 5000;
/// 单轮复习上限范围
pub const MIN_SESSION_LIMIT: i64 = 1;
pub const MAX_SESSION_LIMIT: i64 = 100;

fn errp(code: &str, key: &str, value: impl Into<serde_json::Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

pub fn parse_date(text: &str, code: &str) -> PluginResult<Date> {
    text.parse::<Date>().map_err(|_| errp(code, "value", text))
}

pub fn add_days(date: Date, days: i64) -> PluginResult<Date> {
    date.checked_add(jiff::Span::new().days(days as i32))
        .map_err(|_| PluginError::new("time.out_of_range"))
}

/// 连续记住次数 → 下次间隔天数（streak 从 1 起算，封顶 30 天）
pub fn interval_for(streak: i64) -> i64 {
    let index = (streak.min(STAGES.len() as i64) - 1).max(0) as usize;
    STAGES[index]
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewArgs {
    /// 评分前的连续记住次数（0 = 新卡或忘记后）
    pub stage: i64,
    /// 评分日（本地日历日）"YYYY-MM-DD"
    pub today: String,
    pub remembered: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewOut {
    /// 评分后的连续记住次数
    pub stage: i64,
    /// 下次复习日 "YYYY-MM-DD"；忘记后 = 今天 + 1
    pub due: String,
    /// 本次安排的间隔天数（忘记 = 1）
    pub interval: i64,
    /// 评分前状态（撤销用）
    pub prev_stage: i64,
    pub prev_due: String,
}

/// 对一张卡评分：remembered = 连续 +1 并按档位顺延；forgot = 清零并安排明天
pub fn review(args: ReviewArgs) -> PluginResult<ReviewOut> {
    if args.stage < 0 {
        return Err(errp("fcs.bad_stage", "value", args.stage));
    }
    let today = parse_date(&args.today, "fcs.bad_date")?;
    let (stage, interval) = if args.remembered {
        let streak = args.stage + 1;
        (streak, interval_for(streak))
    } else {
        (0, 1)
    };
    let due = add_days(today, interval)?;
    Ok(ReviewOut {
        stage,
        due: due.to_string(),
        interval,
        prev_stage: args.stage,
        prev_due: String::new(),
    })
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QueueCard {
    pub id: String,
    /// 牌组名（队列本身不分组，仅保留用于统计与排序稳定性）
    #[serde(default)]
    #[allow(dead_code)]
    pub deck: String,
    /// 连续记住次数；0 且 due 为空表示未安排过的新卡
    pub stage: i64,
    /// "YYYY-MM-DD"，空串 = 新卡
    #[serde(default)]
    pub due: String,
    #[serde(default)]
    pub paused: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueArgs {
    pub cards: Vec<QueueCard>,
    /// 今天（本地日历日）
    pub today: String,
    /// 单轮总上限（1..=100）
    pub limit: i64,
    /// 单轮新卡上限（0..=limit）
    pub new_limit: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueCounts {
    pub total: i64,
    pub paused: i64,
    pub due: i64,
    pub new: i64,
    /// 本轮给出的卡片数
    pub selected: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueOut {
    /// 本轮到期卡 id（按到期日升序，超上限截断）
    pub due_ids: Vec<String>,
    /// 本轮新卡 id（按输入顺序，超上限截断）
    pub new_ids: Vec<String>,
    /// 下一个到期日（无到期卡时为空）
    pub next_due: String,
    pub counts: QueueCounts,
}

/// 构建本轮复习队列：到期卡（due ≤ 今天）优先，其次新卡；
/// 跳过与未展示不算已复习——这里只负责挑选，评分必须走 review。
pub fn queue(args: QueueArgs) -> PluginResult<QueueOut> {
    if !(MIN_SESSION_LIMIT..=MAX_SESSION_LIMIT).contains(&args.limit) {
        return Err(errp("fcs.limit_range", "max", MAX_SESSION_LIMIT));
    }
    if args.new_limit < 0 || args.new_limit > args.limit {
        return Err(errp("fcs.new_limit_range", "max", args.limit));
    }
    if args.cards.len() > MAX_CARDS {
        return Err(errp("fcs.too_many_cards", "max", MAX_CARDS));
    }
    let today = parse_date(&args.today, "fcs.bad_date")?;
    let mut due_cards: Vec<(Date, &QueueCard)> = Vec::new();
    let mut new_cards: Vec<&QueueCard> = Vec::new();
    let mut paused = 0i64;
    for card in &args.cards {
        if card.paused {
            paused += 1;
            continue;
        }
        if card.due.is_empty() {
            if card.stage == 0 {
                new_cards.push(card);
            }
            continue;
        }
        let due = parse_date(&card.due, "fcs.bad_date")?;
        if due <= today {
            due_cards.push((due, card));
        }
    }
    due_cards.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.id.cmp(&b.1.id)));
    let due_total = due_cards.len() as i64;
    let new_total = new_cards.len() as i64;
    // 总上限优先给到期卡，剩余名额给新卡
    let due_selected = due_total.min(args.limit);
    let new_selected = new_total.min(args.new_limit).min(args.limit - due_selected);
    // 下一个到期日：今天之后最早的到期日（预告未来负载）
    let future_due = args
        .cards
        .iter()
        .filter(|card| !card.paused && !card.due.is_empty())
        .filter_map(|card| parse_date(&card.due, "fcs.bad_date").ok())
        .filter(|due| *due > today)
        .min()
        .map(|due| due.to_string())
        .unwrap_or_default();
    Ok(QueueOut {
        due_ids: due_cards
            .iter()
            .take(due_selected as usize)
            .map(|(_, card)| card.id.clone())
            .collect(),
        new_ids: new_cards
            .iter()
            .take(new_selected as usize)
            .map(|card| card.id.clone())
            .collect(),
        next_due: future_due,
        counts: QueueCounts {
            total: args.cards.len() as i64,
            paused,
            due: due_total,
            new: new_total,
            selected: due_selected + new_selected,
        },
    })
}

#[cfg(test)]
#[path = "schedule_test.rs"]
mod tests;
