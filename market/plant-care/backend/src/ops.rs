use crate::model::*;
use serde::Deserialize;
use tf_plugin_api::{PluginResult, TaskContext};
#[derive(Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Op {
    NewPlant {
        name: String,
        variety: String,
        location: String,
        note: String,
    },
    EditPlant {
        plant: String,
        name: String,
        variety: String,
        location: String,
        note: String,
    },
    Select {
        plant: String,
    },
    Archive {
        plant: String,
        archived: bool,
    },
    DeletePlant {
        plant: String,
        confirm: bool,
    },
    NewPlan {
        plant: String,
        name: String,
        first: String,
        interval: String,
    },
    EditPlan {
        plant: String,
        plan: String,
        name: String,
        first: String,
        interval: String,
    },
    Pause {
        plant: String,
        plan: String,
        paused: bool,
    },
    Postpone {
        plant: String,
        plan: String,
        date: Option<String>,
    },
    Complete {
        plant: String,
        plan: String,
        date: String,
        note: String,
    },
    Void {
        record: String,
        voided: bool,
    },
    AddPhoto {
        plant: String,
        date: String,
        note: String,
        path: String,
    },
    RemovePhoto {
        photo: String,
        confirm: bool,
    },
    Cover {
        plant: String,
        photo: Option<String>,
    },
}
fn plan_mut<'a>(g: &'a mut Garden, plant: &str, plan: &str) -> PluginResult<&'a mut Plan> {
    g.plant_mut(plant)?
        .plans
        .iter_mut()
        .find(|p| p.id == plan)
        .ok_or_else(|| error("missing"))
}
fn plan<'a>(g: &'a Garden, plant: &str, plan: &str) -> PluginResult<&'a Plan> {
    g.plant(plant)?
        .plans
        .iter()
        .find(|p| p.id == plan)
        .ok_or_else(|| error("missing"))
}
fn actual(date: &str, today: &str) -> PluginResult<()> {
    if day(date)? > day(today)? {
        Err(error("future"))
    } else {
        Ok(())
    }
}
fn clear_changed(g: &mut Garden, plant_id: &str, plan_id: &str, before: &str) -> PluginResult<()> {
    let after = g.base(plan(g, plant_id, plan_id)?)?;
    if after != before {
        plan_mut(g, plant_id, plan_id)?.postponed = None;
    }
    Ok(())
}
pub fn apply(
    mut g: Garden,
    op: Op,
    key: &str,
    today: &str,
    ctx: &dyn TaskContext,
) -> PluginResult<Garden> {
    day(today)?;
    check(ctx)?;
    if key.is_empty()
        || key.len() > 120
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        return Err(error("state"));
    }
    match op {
        Op::NewPlant {
            name,
            variety,
            location,
            note,
        } => {
            if g.plants.iter().any(|p| p.id == key) {
                return Ok(g);
            }
            if g.plants.len() >= 100 {
                return Err(error("limits"));
            }
            g.plants.push(Plant {
                id: key.into(),
                name: text(&name, 80, true)?,
                variety: text(&variety, 160, false)?,
                location: text(&location, 80, false)?,
                note: text(&note, 1000, false)?,
                archived: false,
                cover: None,
                plans: Vec::new(),
            });
            g.selected_id = Some(key.into());
        }
        Op::EditPlant {
            plant,
            name,
            variety,
            location,
            note,
        } => {
            let p = g.plant_mut(&plant)?;
            p.name = text(&name, 80, true)?;
            p.variety = text(&variety, 160, false)?;
            p.location = text(&location, 80, false)?;
            p.note = text(&note, 1000, false)?;
        }
        Op::Select { plant } => {
            g.plant(&plant)?;
            g.selected_id = Some(plant);
        }
        Op::Archive { plant, archived } => {
            g.plant_mut(&plant)?.archived = archived;
        }
        Op::DeletePlant { plant, confirm } => {
            if !confirm {
                return Err(error("confirm"));
            }
            g.plant(&plant)?;
            g.plants.retain(|p| p.id != plant);
            g.records.retain(|r| r.plant_id != plant);
            g.photos.retain(|r| r.plant_id != plant);
            if g.selected_id.as_deref() == Some(&plant) {
                g.selected_id = g.plants.last().map(|p| p.id.clone());
            }
        }
        Op::NewPlan {
            plant,
            name,
            first,
            interval: value,
        } => {
            if g.plants.iter().flat_map(|p| &p.plans).any(|p| p.id == key) {
                return Ok(g);
            }
            day(&first)?;
            let p = g.plant_mut(&plant)?;
            if p.plans.len() >= 12 {
                return Err(error("limits"));
            }
            p.plans.push(Plan {
                id: key.into(),
                name: text(&name, 80, true)?,
                first,
                interval: interval(&value)?,
                paused: false,
                postponed: None,
            });
        }
        Op::EditPlan {
            plant,
            plan: plan_id,
            name,
            first,
            interval: value,
        } => {
            day(&first)?;
            let p = plan_mut(&mut g, &plant, &plan_id)?;
            let next = interval(&value)?;
            if first != p.first || next != p.interval {
                p.postponed = None;
            }
            p.name = text(&name, 80, true)?;
            p.first = first;
            p.interval = next;
        }
        Op::Pause {
            plant,
            plan,
            paused,
        } => {
            plan_mut(&mut g, &plant, &plan)?.paused = paused;
        }
        Op::Postpone {
            plant,
            plan: plan_id,
            date,
        } => {
            let base = g.base(plan(&g, &plant, &plan_id)?)?;
            let value = if let Some(date) = date {
                if day(&date)? < day(today)? || date <= base {
                    return Err(error("postpone"));
                }
                Some(Postpone { base, until: date })
            } else {
                None
            };
            plan_mut(&mut g, &plant, &plan_id)?.postponed = value;
        }
        Op::Complete {
            plant,
            plan: plan_id,
            date,
            note,
        } => {
            let note = text(&note, 1000, false)?;
            if let Some(r) = g.records.iter().find(|r| r.id == key) {
                if r.plant_id == plant && r.plan_id == plan_id && r.date == date && r.note == note {
                    return Ok(g);
                }
                return Err(error("duplicate"));
            }
            actual(&date, today)?;
            let p = g.plant(&plant)?;
            let care = plan(&g, &plant, &plan_id)?;
            if p.archived || care.paused {
                return Err(error("inactive"));
            }
            if g.records.len() >= MAX_RECORDS {
                return Err(error("limits"));
            }
            let before = g.base(care)?;
            g.records.push(Record {
                id: key.into(),
                plant_id: plant.clone(),
                plan_id: plan_id.clone(),
                date,
                note,
                voided: false,
            });
            clear_changed(&mut g, &plant, &plan_id, &before)?;
        }
        Op::Void { record, voided } => {
            let r = g
                .records
                .iter()
                .find(|r| r.id == record)
                .ok_or_else(|| error("missing"))?;
            if !voided {
                actual(&r.date, today)?;
            }
            let (plant_id, plan_id) = (r.plant_id.clone(), r.plan_id.clone());
            let before = g.base(plan(&g, &plant_id, &plan_id)?)?;
            g.records
                .iter_mut()
                .find(|r| r.id == record)
                .ok_or_else(|| error("missing"))?
                .voided = voided;
            clear_changed(&mut g, &plant_id, &plan_id, &before)?;
        }
        Op::AddPhoto {
            plant,
            date,
            note,
            path,
        } => {
            if g.photos.iter().any(|p| p.id == key) {
                return Ok(g);
            }
            actual(&date, today)?;
            g.plant(&plant)?;
            if g.photos.len() >= MAX_PHOTOS {
                return Err(error("limits"));
            }
            let photo = crate::photos::import(
                key.into(),
                plant.clone(),
                date,
                text(&note, 1000, false)?,
                &path,
                ctx,
            )?;
            g.photos.push(photo);
            let p = g.plant_mut(&plant)?;
            if p.cover.is_none() {
                p.cover = Some(key.into());
            }
        }
        Op::RemovePhoto { photo, confirm } => {
            if !confirm {
                return Err(error("confirm"));
            }
            if !g.photos.iter().any(|p| p.id == photo) {
                return Err(error("missing"));
            }
            g.photos.retain(|p| p.id != photo);
            for p in &mut g.plants {
                if p.cover.as_deref() == Some(&photo) {
                    p.cover = None;
                }
            }
        }
        Op::Cover { plant, photo } => {
            if photo
                .as_ref()
                .is_some_and(|id| !g.photos.iter().any(|p| &p.id == id && p.plant_id == plant))
            {
                return Err(error("missing"));
            }
            g.plant_mut(&plant)?.cover = photo;
        }
    }
    g.revision = g
        .revision
        .checked_add(1)
        .filter(|n| *n <= 9_007_199_254_740_991)
        .ok_or_else(|| error("limits"))?;
    validate(&g, ctx)?;
    Ok(g)
}
