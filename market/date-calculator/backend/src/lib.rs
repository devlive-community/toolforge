//! 日期计算插件后端：两个日期之间的天数与年月日、工作日（含中国法定节假日与调休），
//! 日期加减与按工作日推算，带农历、节气、节日、干支、月相与宜忌的月历，生肖星座查询与农历纪念日倒计时。

mod calc;
mod calendar;
mod holidays;
mod parse;
mod scheme;
mod zodiac;

use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

use calc::Amount;
use scheme::{CalendarArg, Scheme};

const MANIFEST: &str = include_str!("../../manifest.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiffArgs {
    start: String,
    end: String,
    #[serde(default)]
    include_end: bool,
    /// 额外按这个节假日方案统计工作日
    #[serde(default)]
    scheme: Option<Scheme>,
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
    calendar: CalendarArg,
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
    #[serde(default)]
    scheme: Option<Scheme>,
}

#[derive(Deserialize)]
struct ImportArgs {
    text: String,
}

#[derive(Deserialize)]
struct ExportArgs {
    scheme: Scheme,
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
                let custom = args
                    .scheme
                    .as_ref()
                    .map(scheme::Rules::from_scheme)
                    .transpose()?;
                to_value(calc::difference(
                    parse::moment(&args.start)?,
                    parse::moment(&args.end)?,
                    args.include_end,
                    custom.as_ref(),
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
                    &args.calendar.rules()?,
                )?)
            }
            "info" => {
                let args: DateArgs = parse_args(args)?;
                to_value(calendar::info(parse::date(&args.date)?)?)
            }
            "birth" => to_value(zodiac::birth(parse_args(args)?)?),
            "anniversaries" => to_value(zodiac::anniversaries(parse_args(args)?)?),
            "month" => {
                let args: MonthArgs = parse_args(args)?;
                let rules = args
                    .scheme
                    .as_ref()
                    .map(scheme::Rules::from_scheme)
                    .transpose()?;
                to_value(calendar::month(
                    args.year,
                    args.month,
                    args.monday_first,
                    rules.as_ref(),
                )?)
            }
            "scheme_import" => to_value(scheme::import(&parse_args::<ImportArgs>(args)?.text)?),
            "scheme_export" => to_value(scheme::export(parse_args::<ExportArgs>(args)?.scheme)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(DateCalculator);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
