use crate::date::{format_date, parse_date};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tf_plugin_api::{PluginError, PluginResult, TaskContext};
pub const MAX_STATE: usize = 2 * 1024 * 1024;
pub const MAX_RECORDS: usize = 5000;
pub const MAX_PHOTOS: usize = 40;
pub fn error(code: &str) -> PluginError {
    PluginError::new(format!("plant-care.{code}"))
}
pub fn check(ctx: &dyn TaskContext) -> PluginResult<()> {
    if ctx.is_cancelled() {
        Err(tf_plugin_api::cancelled())
    } else {
        Ok(())
    }
}
pub fn id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}
pub fn text(value: &str, max: usize, required: bool) -> PluginResult<String> {
    let value = value.trim();
    if value.len() > max * 4
        || value.chars().count() > max
        || required && value.is_empty()
        || value
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(error("text"));
    }
    Ok(value.to_owned())
}
pub fn day(value: &str) -> PluginResult<i64> {
    parse_date(value).ok_or_else(|| error("date"))
}
pub fn interval(value: &str) -> PluginResult<u32> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(error("interval"));
    }
    let n = value.parse::<u32>().map_err(|_| error("interval"))?;
    if !(1..=3650).contains(&n) {
        return Err(error("interval"));
    }
    Ok(n)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Postpone {
    pub base: String,
    pub until: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Plan {
    pub id: String,
    pub name: String,
    pub first: String,
    pub interval: u32,
    pub paused: bool,
    pub postponed: Option<Postpone>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Plant {
    pub id: String,
    pub name: String,
    pub variety: String,
    pub location: String,
    pub note: String,
    pub archived: bool,
    pub cover: Option<String>,
    pub plans: Vec<Plan>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Record {
    pub id: String,
    pub plant_id: String,
    pub plan_id: String,
    pub date: String,
    pub note: String,
    pub voided: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Photo {
    pub id: String,
    pub plant_id: String,
    pub date: String,
    pub note: String,
    pub data: String,
    pub thumbnail: String,
    pub width: u32,
    pub height: u32,
    pub bytes: usize,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Garden {
    pub schema_version: u32,
    pub revision: u64,
    pub selected_id: Option<String>,
    pub plants: Vec<Plant>,
    pub records: Vec<Record>,
    pub photos: Vec<Photo>,
}
impl Default for Garden {
    fn default() -> Self {
        Self {
            schema_version: 1,
            revision: 0,
            selected_id: None,
            plants: Vec::new(),
            records: Vec::new(),
            photos: Vec::new(),
        }
    }
}
impl Garden {
    pub fn plant(&self, id: &str) -> PluginResult<&Plant> {
        self.plants
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| error("missing"))
    }
    pub fn plant_mut(&mut self, id: &str) -> PluginResult<&mut Plant> {
        self.plants
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| error("missing"))
    }
    pub fn last(&self, plan: &str) -> Option<&Record> {
        self.records
            .iter()
            .filter(|r| r.plan_id == plan && !r.voided)
            .max_by(|a, b| a.date.cmp(&b.date).then(a.id.cmp(&b.id)))
    }
    pub fn base(&self, plan: &Plan) -> PluginResult<String> {
        let date = if let Some(last) = self.last(&plan.id) {
            day(&last.date)? + i64::from(plan.interval)
        } else {
            day(&plan.first)?
        };
        let date = format_date(date);
        day(&date)?;
        Ok(date)
    }
    pub fn due(&self, plan: &Plan) -> PluginResult<String> {
        let base = self.base(plan)?;
        Ok(plan
            .postponed
            .as_ref()
            .filter(|p| p.base == base)
            .map_or(base, |p| p.until.clone()))
    }
}
fn valid_id(value: &str, ids: &mut BTreeSet<String>) -> PluginResult<()> {
    if value.is_empty()
        || value.len() > 120
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        || !ids.insert(value.into())
    {
        Err(error("state"))
    } else {
        Ok(())
    }
}
pub fn validate(g: &Garden, ctx: &dyn TaskContext) -> PluginResult<()> {
    check(ctx)?;
    if g.schema_version != 1
        || g.revision > 9_007_199_254_740_991
        || g.plants.len() > 100
        || g.records.len() > MAX_RECORDS
        || g.photos.len() > MAX_PHOTOS
    {
        return Err(error("limits"));
    }
    if serde_json::to_vec(g).map_err(|_| error("state"))?.len() > MAX_STATE {
        return Err(error("storage"));
    }
    let mut ids = BTreeSet::new();
    let mut plan_ids = BTreeSet::new();
    for p in &g.plants {
        check(ctx)?;
        valid_id(&p.id, &mut ids)?;
        text(&p.name, 80, true)?;
        text(&p.variety, 160, false)?;
        text(&p.location, 80, false)?;
        text(&p.note, 1000, false)?;
        if p.plans.len() > 12 {
            return Err(error("limits"));
        }
        for plan in &p.plans {
            valid_id(&plan.id, &mut ids)?;
            plan_ids.insert((&p.id, &plan.id));
            text(&plan.name, 80, true)?;
            day(&plan.first)?;
            if !(1..=3650).contains(&plan.interval) {
                return Err(error("interval"));
            }
        }
    }
    for r in &g.records {
        check(ctx)?;
        valid_id(&r.id, &mut ids)?;
        day(&r.date)?;
        text(&r.note, 1000, false)?;
        if !plan_ids.contains(&(&r.plant_id, &r.plan_id)) {
            return Err(error("state"));
        }
    }
    for photo in &g.photos {
        check(ctx)?;
        valid_id(&photo.id, &mut ids)?;
        g.plant(&photo.plant_id)?;
        day(&photo.date)?;
        text(&photo.note, 1000, false)?;
        crate::photos::validate(photo)?;
    }
    for p in &g.plants {
        if p.cover.as_ref().is_some_and(|id| {
            !g.photos
                .iter()
                .any(|photo| &photo.id == id && photo.plant_id == p.id)
        }) {
            return Err(error("state"));
        }
        for plan in &p.plans {
            let base = g.base(plan)?;
            if let Some(post) = &plan.postponed {
                day(&post.base)?;
                day(&post.until)?;
                if post.base != base || post.until <= base {
                    return Err(error("state"));
                }
            }
        }
    }
    if g.plants.is_empty() != g.selected_id.is_none()
        || g.selected_id
            .as_ref()
            .is_some_and(|id| g.plant(id).is_err())
    {
        return Err(error("state"));
    }
    Ok(())
}
