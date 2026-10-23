use crate::model::*;
use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{PluginResult, TaskContext};
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Args {
    pub handle: String,
    pub format: String,
    pub language: String,
    pub path: Option<String>,
}
fn escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\\' | '|' | '[' | ']' | '*' | '_' | '`' | '#' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}
pub fn run(report: &Report, args: Args, ctx: &dyn TaskContext) -> PluginResult<Value> {
    ctx.stage("export");
    let zh = args.language.starts_with("zh");
    let content = match args.format.as_str() {
        "json" => serde_json::to_string_pretty(&json!({"schemaVersion":1,"input":report.input,"bills":report.bills,"annualization":"12 identical billing months","tierAllocation":"proportional energy; display cents allocated by largest remainder","excludes":["taxes","fixed charges","rebates","demand charges","automatic climate or efficiency savings"]})).map_err(|_| error("format"))?,
        "markdown" => {
            let mut out = format!("# {}\n\n{}: {} · {} {}\n\n{}\n\n", if zh { "用电成本估算" } else { "Electricity cost estimate" }, if zh { "币种" } else { "Currency" }, escape(&report.input.currency), report.input.days, if zh { "天／月" } else { "days/month" }, if zh { "年化＝12 个相同用电月。不含税费、固定费用、补贴或需量费。阶梯按全屋月累计，不按单个电器重置；电器电费是按用电比例分摊的账单。" } else { "Annualized = 12 identical billing months. Excludes taxes, fixed charges, rebates and demand charges. Monthly tiers apply to the whole household; appliance costs are proportional allocations." });
            for b in &report.bills {
                check(ctx)?;
                out.push_str(&format!("## {}\n\n{}: {} kWh · {}: {} {} · {}: {} {}\n\n",escape(&b.tariff.name),if zh{"月电量"}else{"Monthly energy"},b.baseline.monthly_kwh,if zh{"月费"}else{"Monthly cost"},b.baseline.monthly_cost,escape(&report.input.currency),if zh{"替代方案月费"}else{"Alternative monthly cost"},b.alternative.monthly_cost,escape(&report.input.currency)));
                out.push_str(if zh { "| 电器 | 原电量 kWh | 原月费 | 替代电量 kWh | 替代月费 |\n|---|---:|---:|---:|---:|\n" } else { "| Appliance | Base kWh | Base cost | Alternative kWh | Alternative cost |\n|---|---:|---:|---:|---:|\n" });
                for row in &b.rows { out.push_str(&format!("| {} | {} | {} | {} | {} |\n",escape(&row.name),row.baseline_kwh,row.baseline_cost,row.alternative_kwh,row.alternative_cost)); }
                out.push('\n');
                out.push_str(&format!("{}: {} ({})\n\n",if zh{"年化差额"}else{"Annualized difference"},b.annual_saving,match(b.saving_direction.as_str(),zh){("saving",true)=>"减少",("increase",true)=>"增加",("same",true)=>"不变",("saving",false)=>"saving",("increase",false)=>"increase",_=>"same"}));
            }
            out
        }
        _ => return Err(error("format")),
    };
    if content.len() > 2 * 1024 * 1024 {
        return Err(error("exportLimit"));
    }
    check(ctx)?;
    if let Some(path) = args.path {
        if path.trim().is_empty() || path.len() > 4096 {
            return Err(error("path"));
        }
        tf_plugin_guest::net::write_file(&path, content.as_bytes())?;
        Ok(json!({"saved":true}))
    } else if content.len() <= 64 * 1024 {
        Ok(json!({"content":content}))
    } else {
        Err(error("copyLimit"))
    }
}
