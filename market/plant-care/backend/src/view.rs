use crate::{date::format_date, model::*};
use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{PluginResult, TaskContext};
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Query {
    #[serde(default)]
    pub section: String,
    #[serde(default)]
    pub offset: usize,
    #[serde(default)]
    pub plants_offset: usize,
    #[serde(default)]
    pub filter: String,
    #[serde(default)]
    pub search: String,
}
fn page(items: &[Value], offset: usize, limit: usize) -> Value {
    let offset = offset.min(items.len().saturating_sub(1)) / limit * limit;
    json!({"items":items.iter().skip(offset).take(limit).collect::<Vec<_>>(),"offset":offset,"limit":limit,"total":items.len()})
}
pub fn inspect(g: &Garden, today: &str, q: &Query, ctx: &dyn TaskContext) -> PluginResult<Value> {
    let today_day = day(today)?;
    let mut tasks = Vec::new();
    let mut overdue = 0;
    let mut due_today = 0;
    for p in &g.plants {
        for plan in &p.plans {
            check(ctx)?;
            let base = g.base(plan)?;
            let due = g.due(plan)?;
            let active = !p.archived && !plan.paused;
            let d = day(&due)?;
            let status = if !active {
                "paused"
            } else if d < today_day {
                overdue += 1;
                "overdue"
            } else if d == today_day {
                due_today += 1;
                "today"
            } else {
                "upcoming"
            };
            tasks.push(json!({"id":plan.id,"plantId":p.id,"plantName":p.name,"name":plan.name,"first":plan.first,"interval":plan.interval,"paused":plan.paused,"archived":p.archived,"baseDue":base,"due":due,"status":status,"days":d-today_day,"last":g.last(&plan.id).map(|r|&r.date),"postponed":plan.postponed.is_some()}));
        }
    }
    tasks.sort_by(|a, b| {
        a["due"]
            .as_str()
            .cmp(&b["due"].as_str())
            .then(a["id"].as_str().cmp(&b["id"].as_str()))
    });
    let selected = g.selected_id.as_deref();
    let active = g.plants.iter().find(|p| Some(p.id.as_str()) == selected);
    let needle = q.search.to_lowercase();
    if needle.chars().count() > 160
        || !matches!(q.filter.as_str(), "" | "active" | "archived" | "all")
    {
        return Err(error("query"));
    }
    let plants:Vec<_>=g.plants.iter().filter(|p|match q.filter.as_str(){"archived"=>p.archived,"all"=>true,_=>!p.archived}).filter(|p|format!("{} {} {}",p.name,p.variety,p.location).to_lowercase().contains(&needle)).map(|p|json!({"id":p.id,"name":p.name,"variety":p.variety,"location":p.location,"archived":p.archived,"thumbnail":p.cover.as_ref().and_then(|id|g.photos.iter().find(|i|&i.id==id)).map(|i|&i.thumbnail)})).collect();
    let mut timeline = Vec::new();
    if let Some(p) = active {
        for r in g.records.iter().filter(|r| r.plant_id == p.id) {
            timeline.push(json!({"id":r.id,"kind":"care","date":r.date,"note":r.note,"name":p.plans.iter().find(|plan|plan.id==r.plan_id).map(|plan|&plan.name),"voided":r.voided}));
        }
        for photo in g.photos.iter().filter(|r| r.plant_id == p.id) {
            timeline.push(json!({"id":photo.id,"kind":"photo","date":photo.date,"note":photo.note,"thumbnail":photo.thumbnail,"width":photo.width,"height":photo.height,"bytes":photo.bytes}));
        }
    }
    timeline.sort_by(|a, b| {
        b["date"]
            .as_str()
            .cmp(&a["date"].as_str())
            .then(b["id"].as_str().cmp(&a["id"].as_str()))
    });
    let items: Vec<_> = match q.section.as_str() {
        "" | "today" => tasks
            .iter()
            .filter(|r| ["overdue", "today"].contains(&r["status"].as_str().unwrap_or_default()))
            .cloned()
            .collect(),
        "plans" => tasks
            .iter()
            .filter(|r| r["plantId"].as_str() == selected)
            .cloned()
            .collect(),
        "timeline" => timeline,
        "gallery" => timeline
            .into_iter()
            .filter(|r| r["kind"] == "photo")
            .collect(),
        _ => return Err(error("query")),
    };
    let stored_bytes = serde_json::to_vec(g).map_err(|_| error("state"))?.len();
    Ok(
        json!({"today":format_date(today_day),"selected":active,"plants":page(&plants,q.plants_offset,8),"page":page(&items,q.offset,10),"counts":{"plants":g.plants.len(),"overdue":overdue,"today":due_today,"photos":g.photos.len(),"records":g.records.len(),"storedBytes":stored_bytes,"storageLimit":MAX_STATE}}),
    )
}
