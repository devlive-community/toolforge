//! 手账数据结构、校验与上限。
//!
//! 作品资料与每次阅读/观影经历分开保存：
//! - 作品（Work）只描述「是什么」：名称、作者/导演、年份、类型、标签、状态、总页数/总分钟数；
//! - 经历（Session）描述「这一次」：开始/完成日期、当前进度、评分与短评。重读/重看新增一次经历，
//!   旧经历原样保留；修改作品资料不会改动任何经历里的评价。
//!
//! 进度与完成分开确认：进度到了最后一页也不会自动算作完成，只有明确「完成」才计入完成次数。

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::date::parse_date;

pub const SCHEMA: u32 = 1;
pub const MAX_WORKS: usize = 2000;
pub const MAX_SESSIONS: usize = 5000;
pub const MAX_TITLE: usize = 120;
pub const MAX_FIELD: usize = 80;
pub const MAX_REVIEW: usize = 2000;
pub const MAX_TAGS: usize = 10;
pub const MAX_TAG: usize = 30;
/// 总页数 / 总分钟数上限
pub const MAX_TOTAL: u32 = 100_000;
/// 评分为 1–10（界面按半星显示 0.5–5 星）
pub const MAX_RATING: u8 = 10;
/// 序列化后的手账上限，低于宿主单个状态键 2 MiB
pub const MAX_JOURNAL_BYTES: usize = 1_800_000;
pub const RECENT_OPS: usize = 100;

pub fn error(code: &str) -> PluginError {
    PluginError::new(format!("journal.{code}"))
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    #[default]
    Book,
    Film,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    /// 想读 / 想看
    #[default]
    Want,
    /// 进行中
    Doing,
    /// 已完成
    Done,
    /// 暂时放下
    Paused,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Journal {
    pub schema: u32,
    pub revision: u64,
    pub works: Vec<Work>,
    pub sessions: Vec<Session>,
    pub recent_ops: Vec<String>,
    pub undo: Option<Undo>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Work {
    pub id: String,
    pub kind: Kind,
    pub title: String,
    /// 作者或导演
    pub creator: String,
    pub year: Option<i32>,
    pub genre: String,
    pub tags: Vec<String>,
    pub status: Status,
    /// 总页数（书）或总分钟数（电影）；未知为 None
    pub total: Option<u32>,
    pub notes: String,
    pub has_cover: bool,
    pub created: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Session {
    pub id: String,
    pub work_id: String,
    pub start: Option<String>,
    pub finish: Option<String>,
    /// 已读页数 / 已看分钟数
    pub progress: u32,
    /// 明确确认完成后为 true；缺少完成日的完成经历单列待补
    pub completed: bool,
    pub rating: Option<u8>,
    pub review: String,
    pub created: String,
}

/// 撤销最近一步：受影响作品与经历的旧版本（None 表示操作前不存在）
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Undo {
    pub kind: String,
    pub work_id: Option<String>,
    pub work: Option<Work>,
    pub session_id: Option<String>,
    pub session: Option<Session>,
}

impl Journal {
    pub fn work(&self, id: &str) -> PluginResult<&Work> {
        self.works
            .iter()
            .find(|w| w.id == id)
            .ok_or_else(|| error("work_not_found"))
    }

    pub fn session(&self, id: &str) -> PluginResult<&Session> {
        self.sessions
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| error("session_not_found"))
    }

    pub fn sessions_of<'a>(&'a self, work_id: &'a str) -> impl Iterator<Item = &'a Session> + 'a {
        self.sessions.iter().filter(move |s| s.work_id == work_id)
    }
}

pub fn text_ok(text: &str, max: usize) -> bool {
    text.chars().count() <= max && !text.chars().any(|c| c.is_control() && c != '\n')
}

pub fn id_ok(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

pub fn check_work(work: &Work) -> PluginResult<()> {
    if !id_ok(&work.id) || parse_date(&work.created).is_none() {
        return Err(error("bad_work"));
    }
    if work.title.trim().is_empty() || !text_ok(&work.title, MAX_TITLE) {
        return Err(error("bad_title"));
    }
    if !text_ok(&work.creator, MAX_FIELD)
        || !text_ok(&work.genre, MAX_FIELD)
        || !text_ok(&work.notes, MAX_REVIEW)
    {
        return Err(error("text_too_long"));
    }
    if work.year.is_some_and(|y| !(-3000..=9999).contains(&y)) {
        return Err(error("bad_year"));
    }
    if work.total.is_some_and(|t| t == 0 || t > MAX_TOTAL) {
        return Err(error("bad_total").with("max", MAX_TOTAL));
    }
    if work.tags.len() > MAX_TAGS
        || work
            .tags
            .iter()
            .any(|t| t.trim().is_empty() || !text_ok(t, MAX_TAG))
        || work.tags.iter().collect::<BTreeSet<_>>().len() != work.tags.len()
    {
        return Err(error("bad_tags").with("max", MAX_TAGS));
    }
    Ok(())
}

/// 经历检查：日期顺序、进度不超过已知总量、评分范围、短评长度
pub fn check_session(session: &Session, work: &Work) -> PluginResult<()> {
    if !id_ok(&session.id) || parse_date(&session.created).is_none() {
        return Err(error("bad_session"));
    }
    let start = match &session.start {
        Some(d) => Some(parse_date(d).ok_or_else(|| error("bad_date"))?),
        None => None,
    };
    let finish = match &session.finish {
        Some(d) => Some(parse_date(d).ok_or_else(|| error("bad_date"))?),
        None => None,
    };
    if let (Some(s), Some(f)) = (start, finish)
        && f < s
    {
        return Err(error("finish_before_start"));
    }
    if finish.is_some() && !session.completed {
        return Err(error("finish_without_complete"));
    }
    if let Some(total) = work.total
        && session.progress > total
    {
        return Err(error("over_total").with("total", total));
    }
    if session.progress > MAX_TOTAL {
        return Err(error("bad_total").with("max", MAX_TOTAL));
    }
    if session.rating.is_some_and(|r| r == 0 || r > MAX_RATING) {
        return Err(error("bad_rating"));
    }
    if !text_ok(&session.review, MAX_REVIEW) {
        return Err(error("review_too_long").with("max", MAX_REVIEW));
    }
    Ok(())
}

pub fn validate(journal: &Journal) -> PluginResult<()> {
    if journal.schema != SCHEMA {
        return Err(error("bad_schema"));
    }
    if journal.works.len() > MAX_WORKS || journal.sessions.len() > MAX_SESSIONS {
        return Err(error("too_many"));
    }
    let mut ids = BTreeSet::new();
    for work in &journal.works {
        check_work(work)?;
        if !ids.insert(work.id.as_str()) {
            return Err(error("duplicate_id"));
        }
    }
    for session in &journal.sessions {
        let work = journal
            .work(&session.work_id)
            .map_err(|_| error("broken_link"))?;
        check_session(session, work)?;
        if !ids.insert(session.id.as_str()) {
            return Err(error("duplicate_id"));
        }
    }
    if journal.recent_ops.len() > RECENT_OPS {
        return Err(error("bad_journal"));
    }
    Ok(())
}

pub fn check_size(journal: &Journal) -> PluginResult<()> {
    let bytes = serde_json::to_vec(journal)
        .map_err(|_| error("bad_journal"))?
        .len();
    if bytes > MAX_JOURNAL_BYTES {
        return Err(error("journal_full"));
    }
    Ok(())
}
