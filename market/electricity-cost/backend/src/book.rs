use crate::model::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tf_plugin_api::PluginResult;
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Book {
    pub schema_version: u32,
    pub revision: u64,
    pub tariffs: Vec<Tariff>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Args {
    pub state: Book,
    pub tariff: Option<Tariff>,
    pub remove: Option<String>,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "limit")]
    pub limit: usize,
}
fn limit() -> usize {
    5
}
pub fn run(mut args: Args) -> PluginResult<Value> {
    if args.state.schema_version != 1
        || args.state.tariffs.len() > 40
        || args.limit == 0
        || args.limit > 20
        || (args.remove.is_some() && args.tariff.is_some())
    {
        return Err(error("book"));
    }
    let mut ids = std::collections::HashSet::new();
    for t in &args.state.tariffs {
        validate_tariff(t)?;
        if !ids.insert(&t.id) {
            return Err(error("book"));
        }
    }
    let mut changed = false;
    if let Some(tariff) = args.tariff {
        validate_tariff(&tariff)?;
        if let Some(t) = args.state.tariffs.iter_mut().find(|t| t.id == tariff.id) {
            *t = tariff;
        } else {
            args.state.tariffs.push(tariff);
        }
        changed = true;
    }
    if let Some(id) = args.remove {
        let n = args.state.tariffs.len();
        args.state.tariffs.retain(|t| t.id != id);
        changed = n != args.state.tariffs.len();
    }
    if args.state.tariffs.len() > 40 {
        return Err(error("book"));
    }
    if changed {
        args.state.revision = args
            .state
            .revision
            .checked_add(1)
            .ok_or_else(|| error("book"))?;
    }
    let size = serde_json::to_vec(&args.state)
        .map_err(|_| error("book"))?
        .len();
    if size > 128 * 1024 {
        return Err(error("book"));
    }
    let total = args.state.tariffs.len();
    let offset = args.offset.min(total.saturating_sub(1)) / args.limit * args.limit;
    Ok(
        json!({"state":args.state,"page":{"items":args.state.tariffs.iter().skip(offset).take(args.limit).collect::<Vec<_>>(),"total":total,"offset":offset,"limit":args.limit}}),
    )
}
