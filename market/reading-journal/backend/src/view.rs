//! 只读视图：作品筛选与分页、单部作品的全部经历、年度回顾与待补列表。
//!
//! 年度回顾只按「实际完成日」统计：同一部作品当年重读两次算 1 部作品、2 次完成；
//! 已完成但缺少完成日的经历不进入任何年份，单列待补，不用录入日冒充。

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use tf_plugin_api::PluginResult;

use crate::date::parse_date;
use crate::model::*;

const MAX_PAGE: usize = 100;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    pub journal: Journal,
    #[serde(default)]
    pub search: String,
    #[serde(default)]
    pub kind: Option<Kind>,
    #[serde(default)]
    pub status: Option<Status>,
    #[serde(default)]
    pub tag: Option<String>,
    /// 作品年份
    #[serde(default)]
    pub year: Option<i32>,
    /// 任一经历评分不低于它（1–10）
    #[serde(default)]
    pub min_rating: Option<u8>,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "page")]
    pub limit: usize,
}

fn page() -> usize {
    20
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkView {
    #[serde(flatten)]
    pub work: Work,
    pub sessions: usize,
    pub completions: usize,
    /// 最近一次经历的进度（没有经历时为 None）
    pub progress: Option<u32>,
    pub best_rating: Option<u8>,
    pub last_finish: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    #[serde(flatten)]
    pub session: Session,
    pub title: String,
    pub kind: Kind,
    pub total: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub want: usize,
    pub doing: usize,
    pub done: usize,
    pub paused: usize,
    pub sessions: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub revision: u64,
    pub works: Vec<WorkView>,
    pub total: usize,
    pub counts: Counts,
    pub tags: Vec<String>,
    /// 有完成记录的年份（新到旧）
    pub review_years: Vec<i32>,
    /// 已完成但缺少完成日的经历
    pub pending: Vec<SessionView>,
    pub undo: Option<String>,
}

fn latest<'a>(journal: &'a Journal, work_id: &'a str) -> Option<&'a Session> {
    journal
        .sessions_of(work_id)
        .max_by(|a, b| a.created.cmp(&b.created).then_with(|| a.id.cmp(&b.id)))
}

fn work_view(journal: &Journal, work: &Work) -> WorkView {
    let sessions: Vec<&Session> = journal.sessions_of(&work.id).collect();
    WorkView {
        work: work.clone(),
        sessions: sessions.len(),
        completions: sessions.iter().filter(|s| s.completed).count(),
        progress: latest(journal, &work.id).map(|s| s.progress),
        best_rating: sessions.iter().filter_map(|s| s.rating).max(),
        last_finish: sessions.iter().filter_map(|s| s.finish.clone()).max(),
    }
}

fn session_view(journal: &Journal, session: &Session) -> SessionView {
    let work = journal.work(&session.work_id).ok();
    SessionView {
        session: session.clone(),
        title: work.map(|w| w.title.clone()).unwrap_or_default(),
        kind: work.map(|w| w.kind).unwrap_or_default(),
        total: work.and_then(|w| w.total),
    }
}

fn matches(journal: &Journal, work: &Work, words: &[String]) -> bool {
    if words.is_empty() {
        return true;
    }
    let mut hay = [&work.title, &work.creator, &work.genre, &work.notes]
        .map(|s| s.to_lowercase())
        .join(" ");
    for tag in &work.tags {
        hay.push(' ');
        hay.push_str(&tag.to_lowercase());
    }
    for s in journal.sessions_of(&work.id) {
        hay.push(' ');
        hay.push_str(&s.review.to_lowercase());
    }
    words.iter().all(|w| hay.contains(w))
}

fn year_of(date: &str) -> Option<i32> {
    parse_date(date)?;
    date.get(0..4)?.parse().ok()
}

pub fn view(q: &Query) -> PluginResult<View> {
    let journal = &q.journal;
    validate(journal)?;
    let words: Vec<String> = q
        .search
        .to_lowercase()
        .split_whitespace()
        .map(String::from)
        .collect();
    let mut list: Vec<WorkView> = journal
        .works
        .iter()
        .filter(|w| q.kind.is_none_or(|k| w.kind == k))
        .filter(|w| q.status.is_none_or(|s| w.status == s))
        .filter(|w| {
            q.tag
                .as_deref()
                .is_none_or(|t| w.tags.iter().any(|x| x == t))
        })
        .filter(|w| q.year.is_none_or(|y| w.year == Some(y)))
        .filter(|w| matches(journal, w, &words))
        .map(|w| work_view(journal, w))
        .filter(|v| {
            q.min_rating
                .is_none_or(|m| v.best_rating.is_some_and(|r| r >= m))
        })
        .collect();
    // 最近有动静的在前：最近一次完成日、其次登记日
    list.sort_by(|a, b| {
        b.last_finish
            .cmp(&a.last_finish)
            .then_with(|| b.work.created.cmp(&a.work.created))
            .then_with(|| a.work.title.cmp(&b.work.title))
    });
    let total = list.len();
    let limit = q.limit.clamp(1, MAX_PAGE);
    let works = list.into_iter().skip(q.offset).take(limit).collect();

    let mut counts = Counts {
        sessions: journal.sessions.len(),
        ..Counts::default()
    };
    for w in &journal.works {
        match w.status {
            Status::Want => counts.want += 1,
            Status::Doing => counts.doing += 1,
            Status::Done => counts.done += 1,
            Status::Paused => counts.paused += 1,
        }
    }
    let tags: BTreeSet<String> = journal
        .works
        .iter()
        .flat_map(|w| w.tags.iter().cloned())
        .collect();
    let years: BTreeSet<i32> = journal
        .sessions
        .iter()
        .filter(|s| s.completed)
        .filter_map(|s| s.finish.as_deref().and_then(year_of))
        .collect();
    let mut pending: Vec<SessionView> = journal
        .sessions
        .iter()
        .filter(|s| s.completed && s.finish.is_none())
        .map(|s| session_view(journal, s))
        .collect();
    pending.sort_by(|a, b| {
        a.title
            .cmp(&b.title)
            .then_with(|| a.session.created.cmp(&b.session.created))
    });
    Ok(View {
        revision: journal.revision,
        works,
        total,
        counts,
        tags: tags.into_iter().collect(),
        review_years: years.into_iter().rev().collect(),
        pending,
        undo: journal.undo.as_ref().map(|u| u.kind.clone()),
    })
}

/// 单部作品的全部经历（新到旧）
pub fn history(journal: &Journal, work_id: &str) -> PluginResult<(WorkView, Vec<SessionView>)> {
    validate(journal)?;
    let work = journal.work(work_id)?;
    let mut sessions: Vec<SessionView> = journal
        .sessions_of(work_id)
        .map(|s| session_view(journal, s))
        .collect();
    sessions.sort_by(|a, b| {
        b.session
            .created
            .cmp(&a.session.created)
            .then_with(|| b.session.id.cmp(&a.session.id))
    });
    Ok((work_view(journal, work), sessions))
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewItem {
    pub title: String,
    pub creator: String,
    pub kind: Kind,
    pub finish: String,
    /// 评分换算成 0.5–5 星的文字（未评分为空）
    pub stars: String,
    pub rating: Option<u8>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    pub year: i32,
    /// 当年完成的不同作品数
    pub works: usize,
    /// 当年的完成次数（重读、重看各算一次）
    pub completions: usize,
    pub books: usize,
    pub films: usize,
    /// 已知总量的完成经历累计页数 / 分钟数
    pub pages: u64,
    pub minutes: u64,
    /// 平均评分（1–10），没有评分时为 None
    pub average_rating: Option<f64>,
    /// 每月完成次数
    pub months: [usize; 12],
    pub items: Vec<ReviewItem>,
    /// 全部已完成但缺少完成日的经历数（不计入任何年份）
    pub pending: usize,
}

pub fn stars(rating: Option<u8>) -> String {
    rating
        .map(|r| {
            let whole = usize::from(r / 2);
            format!("{}{}", "★".repeat(whole), if r % 2 == 1 { "½" } else { "" })
        })
        .unwrap_or_default()
}

pub fn review(journal: &Journal, year: i32) -> PluginResult<Review> {
    validate(journal)?;
    let mut items = Vec::new();
    let mut works = BTreeSet::new();
    let mut kinds = BTreeSet::new();
    let (mut pages, mut minutes) = (0u64, 0u64);
    let mut ratings = Vec::new();
    let mut months = [0usize; 12];
    for s in journal.sessions.iter().filter(|s| s.completed) {
        let Some(finish) = s.finish.as_deref() else {
            continue;
        };
        if year_of(finish) != Some(year) {
            continue;
        }
        let work = journal.work(&s.work_id)?;
        works.insert(work.id.clone());
        kinds.insert((work.kind, work.id.clone()));
        if let Some(total) = work.total {
            match work.kind {
                Kind::Book => pages += u64::from(total),
                Kind::Film => minutes += u64::from(total),
            }
        }
        if let Some(r) = s.rating {
            ratings.push(f64::from(r));
        }
        let month: usize = finish.get(5..7).and_then(|m| m.parse().ok()).unwrap_or(1);
        months[month.clamp(1, 12) - 1] += 1;
        items.push(ReviewItem {
            title: work.title.clone(),
            creator: work.creator.clone(),
            kind: work.kind,
            finish: finish.to_string(),
            stars: stars(s.rating),
            rating: s.rating,
        });
    }
    items.sort_by(|a, b| a.finish.cmp(&b.finish).then_with(|| a.title.cmp(&b.title)));
    let average = (!ratings.is_empty()).then(|| {
        let avg = ratings.iter().sum::<f64>() / ratings.len() as f64;
        (avg * 10.0).round() / 10.0
    });
    Ok(Review {
        year,
        works: works.len(),
        completions: items.len(),
        books: kinds.iter().filter(|(k, _)| *k == Kind::Book).count(),
        films: kinds.iter().filter(|(k, _)| *k == Kind::Film).count(),
        pages,
        minutes,
        average_rating: average,
        months,
        items,
        pending: journal
            .sessions
            .iter()
            .filter(|s| s.completed && s.finish.is_none())
            .count(),
    })
}

#[cfg(test)]
#[path = "view_test.rs"]
mod tests;
