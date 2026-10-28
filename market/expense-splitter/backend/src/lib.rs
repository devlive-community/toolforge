//! 聚餐旅行 AA 分账（在线安装插件）：金额、分摊与汇总全部在 Rust 侧以「分」为
//! 最小单位整数计算。活动数据由前端经 usePluginState 持久化到 ToolForge SQLite，
//! 后端只做无状态计算与报告导出。

mod flat;
mod ledger;

use std::sync::atomic::{AtomicU64, Ordering};

use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

use ledger::{Ledger, Member};

const MANIFEST: &str = include_str!("../../manifest.json");

/// 后端分发的全部函数；宿主只放行 manifest 中声明过的函数，测试据此核对两边一致
#[cfg(test)]
const FUNCTIONS: [&str; 4] = ["new_id", "settle", "export", "split"];

static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettleArgs {
    ledger: Ledger,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportArgs {
    format: Format,
    report: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Format {
    Csv,
    Json,
    Text,
}

/// 短随机 id：纳秒时间 + 计数器，够前端本地引用使用
fn new_id() -> PluginResult<Value> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let counter = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    to_value(json!({ "id": format!("e{nanos:012x}{counter:02x}") }))
}

/// 分 → "12.34"
fn format_cents(cents: i64) -> String {
    ledger::format_cents(cents)
}

fn csv_escape(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_owned()
    }
}

fn report_from(value: &Value) -> PluginResult<ledger::Report> {
    serde_json::from_value(value.clone())
        .map_err(|e| PluginError::new("esp.serialize").with("detail", e.to_string()))
}

/// CSV 报告：成员余额、逐笔分摊、结算与转账建议
fn csv(report: &ledger::Report) -> String {
    let mut out = String::from("type,id,name,value\n");
    for balance in &report.balances {
        out.push_str(&format!(
            "balance,{},{},{},{},{},{},{}\n",
            csv_escape(&balance.id),
            csv_escape(&balance.name),
            format_cents(balance.paid),
            format_cents(balance.owed),
            format_cents(balance.settled_out),
            format_cents(balance.settled_in),
            format_cents(balance.net),
        ));
    }
    for expense in &report.expenses {
        out.push_str(&format!(
            "expense,{},{},{},\n",
            csv_escape(&expense.id),
            csv_escape(&expense.title),
            format_cents(expense.total_cents),
        ));
        for allocation in &expense.allocations {
            out.push_str(&format!(
                "allocation,{},{},{},\n",
                csv_escape(&expense.id),
                csv_escape(&allocation.id),
                format_cents(allocation.cents),
            ));
        }
    }
    for settlement in &report.settlements {
        out.push_str(&format!(
            "settlement,{},{},{},{},{}\n",
            csv_escape(&settlement.id),
            csv_escape(&settlement.from),
            csv_escape(&settlement.to),
            format_cents(settlement.cents),
            csv_escape(settlement.date.as_deref().unwrap_or_default()),
        ));
    }
    for transfer in &report.transfers {
        out.push_str(&format!(
            "transfer,{},{},{},\n",
            csv_escape(&transfer.from),
            csv_escape(&transfer.to),
            format_cents(transfer.cents),
        ));
    }
    out
}

/// 文本摘要：人读的结算表
fn text(report: &ledger::Report, members: &[Member]) -> String {
    let name_of = |id: &str| -> String {
        members
            .iter()
            .find(|m| m.id == id)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| id.to_owned())
    };
    let mut out = String::new();
    out.push_str(&format!("Total: {}\n\n", format_cents(report.total_cents)));
    out.push_str("Balances (paid / owed / settled / net):\n");
    for balance in &report.balances {
        out.push_str(&format!(
            "- {} ({}): paid {}, owed {}, out {}, in {}, net {}\n",
            balance.name,
            balance.id,
            format_cents(balance.paid),
            format_cents(balance.owed),
            format_cents(balance.settled_out),
            format_cents(balance.settled_in),
            format_cents(balance.net),
        ));
    }
    if !report.settlements.is_empty() {
        out.push_str("\nActual settlements:\n");
        for settlement in &report.settlements {
            out.push_str(&format!(
                "- {}{}: {} -> {}: {}\n",
                settlement.id,
                settlement
                    .date
                    .as_ref()
                    .map(|date| format!(" ({date})"))
                    .unwrap_or_default(),
                name_of(&settlement.from),
                name_of(&settlement.to),
                format_cents(settlement.cents),
            ));
        }
    }
    if report.transfers.is_empty() {
        out.push_str("\nNo transfers needed.\n");
    } else {
        out.push_str("\nSuggested transfers (verify before paying):\n");
        for transfer in &report.transfers {
            out.push_str(&format!(
                "- {} -> {}: {}\n",
                name_of(&transfer.from),
                name_of(&transfer.to),
                format_cents(transfer.cents),
            ));
        }
    }
    out
}

pub struct ExpenseSplitter {
    manifest: Manifest,
}

impl Default for ExpenseSplitter {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for ExpenseSplitter {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "new_id" => new_id(),
            "split" => to_value(flat::split(parse_args(args)?)?),
            "settle" => {
                let args: SettleArgs = parse_args(args)?;
                to_value(ledger::settle(&args.ledger)?)
            }
            "export" => {
                let args: ExportArgs = parse_args(args)?;
                let report = report_from(&args.report)?;
                let members: Vec<Member> = report
                    .balances
                    .iter()
                    .map(|balance| Member {
                        id: balance.id.clone(),
                        name: balance.name.clone(),
                    })
                    .collect();
                let content = match args.format {
                    Format::Csv => csv(&report),
                    Format::Json => serde_json::to_string_pretty(&report).map_err(|e| {
                        PluginError::new("esp.serialize").with("detail", e.to_string())
                    })?,
                    Format::Text => text(&report, &members),
                };
                let ext = match args.format {
                    Format::Csv => "csv",
                    Format::Json => "json",
                    Format::Text => "txt",
                };
                to_value(json!({ "content": content, "ext": ext }))
            }
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(ExpenseSplitter);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
