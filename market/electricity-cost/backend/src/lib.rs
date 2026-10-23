mod book;
mod engine;
mod export;
mod model;
use model::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use tf_plugin_api::{
    Manifest, PluginResult, TaskContext, ToolPlugin, parse_args, unknown_function,
};

#[derive(Default)]
struct Cache {
    generation: u64,
    report: Option<(String, Arc<Report>)>,
}
pub struct ElectricityCost {
    manifest: Manifest,
    cache: Mutex<Cache>,
    sequence: AtomicU64,
}
impl Default for ElectricityCost {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(include_str!("../../manifest.json")),
            cache: Mutex::default(),
            sequence: AtomicU64::new(0),
        }
    }
}
fn usage(power: &str, hours: &str) -> Usage {
    Usage {
        power: power.into(),
        unit: "w".into(),
        peak: hours.into(),
        shoulder: "0".into(),
        valley: "0".into(),
    }
}
impl ElectricityCost {
    fn next_id(&self) -> String {
        let n = self.sequence.fetch_add(1, Ordering::Relaxed);
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        format!("e-{t}-{n}")
    }
    fn report(&self, handle: &str) -> PluginResult<Arc<Report>> {
        self.cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .report
            .as_ref()
            .filter(|(h, _)| h == handle)
            .map(|(_, r)| r.clone())
            .ok_or_else(|| error("expired"))
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Query {
    handle: String,
    tariff: usize,
    #[serde(default)]
    offset: usize,
    #[serde(default = "limit")]
    limit: usize,
}
fn limit() -> usize {
    10
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct New {
    kind: String,
    #[serde(default)]
    preset: String,
}
impl ToolPlugin for ElectricityCost {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "schema" => serde_json::from_str(include_str!("../../agent-schema.json"))
                .map_err(|_| error("schema")),
            "defaults" => tf_plugin_api::to_value(Input {
                days: 30,
                currency: "CNY".into(),
                appliances: vec![Appliance {
                    id: self.next_id(),
                    name: "LED".into(),
                    quantity: 6,
                    usage: usage("10", "5"),
                    alternative: None,
                }],
                tariffs: vec![Tariff {
                    id: self.next_id(),
                    name: "Example".into(),
                    pricing: Pricing::Flat { rate: "0.6".into() },
                }],
            }),
            "new" => {
                let n: New = parse_args(args)?;
                let id = self.next_id();
                match n.kind.as_str() {
                    "appliance" => {
                        let (power, hours) = match n.preset.as_str() {
                            "lighting" => ("10", "5"),
                            "computer" => ("150", "6"),
                            "cooling" => ("1000", "4"),
                            "fridge" => ("100", "8"),
                            "custom" => ("100", "1"),
                            _ => return Err(error("preset")),
                        };
                        tf_plugin_api::to_value(Appliance {
                            id,
                            name: n.preset,
                            quantity: 1,
                            usage: usage(power, hours),
                            alternative: None,
                        })
                    }
                    "tariff" => tf_plugin_api::to_value(Tariff {
                        id,
                        name: "Example".into(),
                        pricing: Pricing::Flat { rate: "0.6".into() },
                    }),
                    _ => Err(error("preset")),
                }
            }
            "book" => book::run(parse_args(args)?),
            "editPage" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase", deny_unknown_fields)]
                struct Page {
                    ids: Vec<String>,
                    offset: usize,
                    limit: usize,
                }
                let p: Page = parse_args(args)?;
                if p.ids.len() > 100 || p.limit == 0 || p.limit > 20 || p.ids.iter().any(|v| !id(v))
                {
                    return Err(error("page"));
                }
                let total = p.ids.len();
                let offset = p.offset.min(total.saturating_sub(1)) / p.limit * p.limit;
                Ok(
                    json!({"items":p.ids.iter().skip(offset).take(p.limit).collect::<Vec<_>>(),"total":total,"offset":offset,"limit":p.limit}),
                )
            }
            _ => Err(unknown_function(function)),
        }
    }
    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        check(ctx)?;
        if function == "calculate" {
            let input: Input = parse_args(args)?;
            ctx.log(tf_plugin_api::LogLevel::Info, "started", json!({}));
            let report = engine::calculate(input, ctx)?;
            check(ctx)?;
            let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
            cache.generation = cache
                .generation
                .checked_add(1)
                .ok_or_else(|| error("expired"))?;
            let handle = cache.generation.to_string();
            let summaries:Vec<_>=report.bills.iter().map(|b|json!({"tariff":b.tariff,"baseline":b.baseline,"alternative":b.alternative,"annualSaving":b.annual_saving,"savingDirection":b.saving_direction})).collect();
            let value = json!({"handle":handle,"currency":report.input.currency,"days":report.input.days,"applianceCount":report.input.appliances.len(),"summaries":summaries});
            cache.report = Some((handle, Arc::new(report)));
            ctx.log(tf_plugin_api::LogLevel::Info, "complete", json!({}));
            return Ok(value);
        }
        if function == "export" {
            let a: export::Args = parse_args(args)?;
            let r = self.report(&a.handle)?;
            return export::run(&r, a, ctx);
        }
        if function == "query" {
            let q: Query = parse_args(args)?;
            if q.limit == 0 || q.limit > 100 {
                return Err(error("page"));
            }
            ctx.stage("query");
            let r = self.report(&q.handle)?;
            let b = r.bills.get(q.tariff).ok_or_else(|| error("page"))?;
            let offset = q.offset.min(b.rows.len().saturating_sub(1)) / q.limit * q.limit;
            check(ctx)?;
            return Ok(
                json!({"items":b.rows.iter().skip(offset).take(q.limit).collect::<Vec<_>>(),"total":b.rows.len(),"offset":offset,"limit":q.limit}),
            );
        }
        Err(unknown_function(function))
    }
}
tf_plugin_guest::export_plugin!(ElectricityCost);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
