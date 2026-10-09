//! 日期计算插件后端：两个日期之间的天数与年月日、工作日（含中国法定节假日与调休），
//! 日期加减与按工作日推算，以及带农历、节气、节日的月历。

mod calc;
mod calendar;
mod holidays;
mod parse;

use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

use calc::Amount;
use holidays::Calendar;

const MANIFEST: &str = include_str!("../../manifest.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiffArgs {
    start: String,
    end: String,
    #[serde(default)]
    include_end: bool,
}

#[derive(Deserialize)]
struct AddArgs {
    start: String,
    #[serde(flatten)]
    amount: Amount,
}

#[derive(Deserialize)]
struct WorkdayArgs {
    start: String,
    days: i64,
    #[serde(default)]
    calendar: Calendar,
}

#[derive(Deserialize)]
struct DateArgs {
    date: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MonthArgs {
    year: i16,
    month: i8,
    #[serde(default = "yes")]
    monday_first: bool,
}

fn yes() -> bool {
    true
}

pub struct DateCalculator {
    manifest: Manifest,
}

impl Default for DateCalculator {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for DateCalculator {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "today" => {
                let today = calendar::today();
                Ok(json!({
                    "date": today.strftime("%Y-%m-%d").to_string(),
                    "year": today.year(),
                    "month": today.month(),
                    "holidayDataUntil": holidays::last_year(),
                }))
            }
            "diff" => {
                let args: DiffArgs = parse_args(args)?;
                to_value(calc::difference(
                    parse::moment(&args.start)?,
                    parse::moment(&args.end)?,
                    args.include_end,
                )?)
            }
            "add" => {
                let args: AddArgs = parse_args(args)?;
                to_value(calc::add(parse::moment(&args.start)?, args.amount)?)
            }
            "add_workdays" => {
                let args: WorkdayArgs = parse_args(args)?;
                to_value(calc::add_workdays(
                    parse::date(&args.start)?,
                    args.days,
                    args.calendar,
                )?)
            }
            "info" => {
                let args: DateArgs = parse_args(args)?;
                to_value(calendar::info(parse::date(&args.date)?)?)
            }
            "month" => {
                let args: MonthArgs = parse_args(args)?;
                to_value(calendar::month(args.year, args.month, args.monday_first)?)
            }
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
