use crate::{
    analysis::{self, Query},
    model::*,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use tf_plugin_api::{PluginResult, TaskContext};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Tile {
    name: String,
    kind: &'static str,
    bytes: u64,
    path: Option<String>,
    directory: Option<Vec<String>>,
    count: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}
fn place(
    items: &mut [Tile],
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    ctx: &dyn TaskContext,
) -> PluginResult<()> {
    check(ctx)?;
    if items.is_empty() {
        return Ok(());
    }
    if items.len() == 1 {
        items[0].x = x;
        items[0].y = y;
        items[0].width = w;
        items[0].height = h;
        return Ok(());
    }
    let total = items.iter().map(|t| t.bytes as f64).sum::<f64>();
    let mut sum = 0.0;
    let mut mid = 1;
    for (i, t) in items.iter().enumerate().take(items.len() - 1) {
        sum += t.bytes as f64;
        mid = i + 1;
        if sum >= total / 2.0 {
            break;
        }
    }
    let ratio = sum / total;
    let (left, right) = items.split_at_mut(mid);
    if w >= h {
        place(left, x, y, w * ratio, h, ctx)?;
        place(right, x + w * ratio, y, w * (1.0 - ratio), h, ctx)
    } else {
        place(left, x, y, w, h * ratio, ctx)?;
        place(right, x, y + h * ratio, w, h * (1.0 - ratio), ctx)
    }
}
pub fn tree(s: &Snapshot, q: &Query, ctx: &dyn TaskContext) -> PluginResult<Value> {
    q.validate()?;
    let out = analysis::out(s, q.output.as_deref())?;
    let mut groups = BTreeMap::<(String, bool), Tile>::new();
    let needle = q.search.to_lowercase();
    for (path, c) in &out.inputs {
        check(ctx)?;
        if c.bytes_in_output == 0 || !path.to_lowercase().contains(&needle) {
            continue;
        }
        let parts = path
            .split(['/', '\\'])
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if !parts.starts_with(&q.directory) {
            continue;
        }
        let Some(segment) = parts.get(q.directory.len()) else {
            continue;
        };
        let folder = parts.len() > q.directory.len() + 1 && q.directory.len() < 24;
        let name = if folder {
            segment.clone()
        } else {
            path.clone()
        };
        let key = (name.clone(), folder);
        let item = groups.entry(key).or_insert_with(|| Tile {
            name: if folder && name.is_empty() {
                "/".into()
            } else {
                name
            },
            kind: if folder { "directory" } else { "module" },
            bytes: 0,
            path: if folder { None } else { Some(path.clone()) },
            directory: if folder {
                Some(parts[..q.directory.len() + 1].to_vec())
            } else {
                None
            },
            count: 0,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        });
        item.bytes = add(item.bytes, c.bytes_in_output)?;
        item.count += 1;
    }
    let mut items = groups.into_values().collect::<Vec<_>>();
    let attributed = analysis::sum(out);
    // Unattributed bytes are their own tile, never assigned to a module.
    if q.directory.is_empty() && q.search.is_empty() && out.bytes > attributed {
        items.push(Tile {
            name: String::new(),
            kind: "unattributed",
            bytes: out.bytes - attributed,
            path: None,
            directory: None,
            count: 0,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        });
    }
    items.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.name.cmp(&b.name)));
    let count = items.len();
    if count > 256 {
        let rest = items.split_off(255);
        items.push(Tile {
            name: String::new(),
            kind: "other",
            bytes: rest.iter().map(|i| i.bytes).sum(),
            path: None,
            directory: None,
            count: rest.len(),
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        });
    }
    let basis: u64 = items.iter().map(|i| i.bytes).sum();
    place(&mut items, 0.0, 0.0, 1000.0, 600.0, ctx)?;
    Ok(
        json!({"items":items,"basis":basis,"groups":count,"limited":count>256,"depthLimited":q.directory.len()==24,"outputBytes":out.bytes,"attributed":attributed,"overAttributed":attributed.saturating_sub(out.bytes)}),
    )
}
