//! 手账操作。每次提交带操作编号与版本号：同一编号重复提交只生效一次，版本不符直接拒绝。
//! 只保留最近一步撤销（受影响作品与经历的旧版本）。
//!
//! 完成与进度分开：`Update` 只改进度、评分与短评，不会把经历标成完成；
//! 只有 `Complete` 才计入一次完成（完成日可留空，进入待补列表），`Uncomplete` 撤销完成。

use serde::{Deserialize, Serialize};
use tf_plugin_api::PluginResult;

use crate::date::{format_date, parse_date};
use crate::model::*;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Fields {
    pub kind: Kind,
    pub title: String,
    pub creator: String,
    pub year: Option<i32>,
    pub genre: String,
    pub tags: Vec<String>,
    pub total: Option<u32>,
    pub notes: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Op {
    #[serde(rename_all = "camelCase")]
    CreateWork {
        fields: Fields,
        status: Status,
    },
    #[serde(rename_all = "camelCase")]
    EditWork {
        work_id: String,
        fields: Fields,
    },
    #[serde(rename_all = "camelCase")]
    SetStatus {
        work_id: String,
        status: Status,
    },
    /// 只能删除没有任何经历的作品
    #[serde(rename_all = "camelCase")]
    DeleteWork {
        work_id: String,
    },
    /// 开始一次新的阅读/观影（重读、重看同样新增，不改旧经历）
    #[serde(rename_all = "camelCase")]
    Start {
        work_id: String,
        #[serde(default)]
        start: Option<String>,
    },
    /// 更新进度、评分、短评与日期；不改变完成状态
    #[serde(rename_all = "camelCase")]
    Update {
        session_id: String,
        #[serde(default)]
        start: Option<String>,
        #[serde(default)]
        finish: Option<String>,
        progress: u32,
        #[serde(default)]
        rating: Option<u8>,
        #[serde(default)]
        review: String,
    },
    /// 明确确认完成；完成日可留空（进入待补列表）
    #[serde(rename_all = "camelCase")]
    Complete {
        session_id: String,
        #[serde(default)]
        finish: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Uncomplete {
        session_id: String,
    },
    #[serde(rename_all = "camelCase")]
    DeleteSession {
        session_id: String,
    },
    /// 封面已由界面写入/删除，这里只更新标记（不可撤销）
    #[serde(rename_all = "camelCase")]
    SetCover {
        work_id: String,
        has_cover: bool,
    },
    Undo,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub journal: Journal,
    pub op: Op,
    pub op_id: String,
    pub revision: u64,
    /// 本地今天：开始与完成日期不能晚于今天
    pub today: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub journal: Journal,
    pub duplicate: bool,
    pub created_id: Option<String>,
}

fn derive_id(prefix: char, op_id: &str) -> String {
    let core: String = op_id
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(24)
        .collect();
    format!("{prefix}-{core}")
}

fn kind_of(op: &Op) -> &'static str {
    match op {
        Op::CreateWork { .. } => "createWork",
        Op::EditWork { .. } => "editWork",
        Op::SetStatus { .. } => "setStatus",
        Op::DeleteWork { .. } => "deleteWork",
        Op::Start { .. } => "start",
        Op::Update { .. } => "update",
        Op::Complete { .. } => "complete",
        Op::Uncomplete { .. } => "uncomplete",
        Op::DeleteSession { .. } => "deleteSession",
        Op::SetCover { .. } => "setCover",
        Op::Undo => "undo",
    }
}

/// 日期：可留空；填写时必须有效且不晚于今天
fn past_date(value: Option<String>, today: i64) -> PluginResult<Option<String>> {
    match value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
    {
        None => Ok(None),
        Some(text) => {
            let day = parse_date(&text).ok_or_else(|| error("bad_date"))?;
            if day > today {
                return Err(error("future_date"));
            }
            Ok(Some(format_date(day)))
        }
    }
}

fn work_from(fields: Fields, id: String, status: Status, has_cover: bool, created: String) -> Work {
    Work {
        id,
        kind: fields.kind,
        title: fields.title.trim().to_string(),
        creator: fields.creator.trim().to_string(),
        year: fields.year,
        genre: fields.genre.trim().to_string(),
        tags: fields
            .tags
            .iter()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect(),
        status,
        total: fields.total,
        notes: fields.notes.trim().to_string(),
        has_cover,
        created,
    }
}

fn put_work(journal: &mut Journal, work: Work) {
    match journal.works.iter_mut().find(|w| w.id == work.id) {
        Some(slot) => *slot = work,
        None => journal.works.push(work),
    }
}

fn put_session(journal: &mut Journal, session: Session) {
    match journal.sessions.iter_mut().find(|s| s.id == session.id) {
        Some(slot) => *slot = session,
        None => journal.sessions.push(session),
    }
}

pub fn apply(req: Request) -> PluginResult<Outcome> {
    let Request {
        journal,
        op,
        op_id,
        revision,
        today,
    } = req;
    validate(&journal)?;
    if op_id.is_empty() || op_id.len() > 64 {
        return Err(error("bad_op"));
    }
    if journal.recent_ops.contains(&op_id) {
        return Ok(Outcome {
            journal,
            duplicate: true,
            created_id: None,
        });
    }
    if revision != journal.revision {
        return Err(error("stale"));
    }
    let today_day = parse_date(&today).ok_or_else(|| error("bad_date"))?;
    let today = format_date(today_day);
    let kind = kind_of(&op).to_string();
    let mut next = journal.clone();
    let mut undoable = true;
    let mut work_id: Option<String> = None;
    let mut session_id: Option<String> = None;
    let mut created_id = None;

    match op {
        Op::CreateWork { fields, status } => {
            if next.works.len() >= MAX_WORKS {
                return Err(error("too_many").with("max", MAX_WORKS));
            }
            let id = derive_id('w', &op_id);
            let work = work_from(fields, id.clone(), status, false, today.clone());
            check_work(&work)?;
            next.works.push(work);
            work_id = Some(id.clone());
            created_id = Some(id);
        }
        Op::EditWork {
            work_id: id,
            fields,
        } => {
            let old = next.work(&id)?.clone();
            let work = work_from(
                fields,
                old.id.clone(),
                old.status,
                old.has_cover,
                old.created.clone(),
            );
            check_work(&work)?;
            // 修改总量后，已有经历的进度仍不能超过它（由 validate 统一检查）
            put_work(&mut next, work);
            work_id = Some(id);
        }
        Op::SetStatus {
            work_id: id,
            status,
        } => {
            let mut work = next.work(&id)?.clone();
            work.status = status;
            put_work(&mut next, work);
            work_id = Some(id);
        }
        Op::DeleteWork { work_id: id } => {
            next.work(&id)?;
            if next.sessions_of(&id).next().is_some() {
                return Err(error("delete_with_sessions"));
            }
            next.works.retain(|w| w.id != id);
            work_id = Some(id);
            undoable = false;
        }
        Op::Start { work_id: id, start } => {
            if next.sessions.len() >= MAX_SESSIONS {
                return Err(error("too_many").with("max", MAX_SESSIONS));
            }
            let mut work = next.work(&id)?.clone();
            let sid = derive_id('s', &op_id);
            put_session(
                &mut next,
                Session {
                    id: sid.clone(),
                    work_id: id.clone(),
                    start: past_date(start, today_day)?,
                    created: today.clone(),
                    ..Session::default()
                },
            );
            work.status = Status::Doing;
            put_work(&mut next, work);
            work_id = Some(id);
            session_id = Some(sid.clone());
            created_id = Some(sid);
        }
        Op::Update {
            session_id: sid,
            start,
            finish,
            progress,
            rating,
            review,
        } => {
            let mut session = next.session(&sid)?.clone();
            session.start = past_date(start, today_day)?;
            // 完成日只属于已完成的经历；未完成时不能单独填写
            session.finish = past_date(finish, today_day)?;
            session.progress = progress;
            session.rating = rating;
            session.review = review.trim().to_string();
            work_id = Some(session.work_id.clone());
            put_session(&mut next, session);
            session_id = Some(sid);
        }
        Op::Complete {
            session_id: sid,
            finish,
        } => {
            let mut session = next.session(&sid)?.clone();
            if session.completed {
                return Err(error("already_completed"));
            }
            session.completed = true;
            session.finish = past_date(finish, today_day)?;
            let mut work = next.work(&session.work_id)?.clone();
            work.status = Status::Done;
            // 已知总量时，完成即读到/看到最后
            if let Some(total) = work.total {
                session.progress = total;
            }
            work_id = Some(work.id.clone());
            put_work(&mut next, work);
            put_session(&mut next, session);
            session_id = Some(sid);
        }
        Op::Uncomplete { session_id: sid } => {
            let mut session = next.session(&sid)?.clone();
            if !session.completed {
                return Err(error("not_completed"));
            }
            session.completed = false;
            session.finish = None;
            let mut work = next.work(&session.work_id)?.clone();
            let still_done = next
                .sessions_of(&work.id)
                .any(|s| s.id != sid && s.completed);
            if !still_done && work.status == Status::Done {
                work.status = Status::Doing;
            }
            work_id = Some(work.id.clone());
            put_work(&mut next, work);
            put_session(&mut next, session);
            session_id = Some(sid);
        }
        Op::DeleteSession { session_id: sid } => {
            let session = next.session(&sid)?.clone();
            next.sessions.retain(|s| s.id != sid);
            work_id = Some(session.work_id);
            session_id = Some(sid);
        }
        Op::SetCover {
            work_id: id,
            has_cover,
        } => {
            let mut work = next.work(&id)?.clone();
            work.has_cover = has_cover;
            put_work(&mut next, work);
            work_id = Some(id);
            undoable = false;
        }
        Op::Undo => {
            let step = next.undo.clone().ok_or_else(|| error("nothing_to_undo"))?;
            if let Some(id) = &step.session_id {
                match step.session.clone() {
                    Some(s) => put_session(&mut next, s),
                    None => next.sessions.retain(|s| &s.id != id),
                }
            }
            if let Some(id) = &step.work_id {
                match step.work.clone() {
                    Some(w) => put_work(&mut next, w),
                    None => next.works.retain(|w| &w.id != id),
                }
            }
            undoable = false;
        }
    }

    validate(&next)?;
    next.undo = undoable.then(|| Undo {
        kind,
        work: work_id
            .as_deref()
            .and_then(|id| journal.work(id).ok().cloned()),
        work_id,
        session: session_id
            .as_deref()
            .and_then(|id| journal.session(id).ok().cloned()),
        session_id,
    });
    next.revision = journal.revision + 1;
    next.recent_ops.push(op_id);
    if next.recent_ops.len() > RECENT_OPS {
        next.recent_ops.remove(0);
    }
    check_size(&next)?;
    Ok(Outcome {
        journal: next,
        duplicate: false,
        created_id,
    })
}

#[cfg(test)]
#[path = "ops_test.rs"]
pub(crate) mod tests;
