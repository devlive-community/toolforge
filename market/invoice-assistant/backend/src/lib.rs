//! 发票价税助手插件后端：价税分离/合并计算、中文大写金额转换与报销清单
//! Markdown 生成。全部纯计算，不读取发票文件（OCR 不在首版范围）。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct InvoiceAssistant {
    manifest: Manifest,
}

impl Default for InvoiceAssistant {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for InvoiceAssistant {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "tax_split" => to_value(tax_split(parse_args(args)?)?),
            "uppercase" => {
                let args: UppercaseArgs = parse_args(args)?;
                to_value(uppercase(args.amount)?)
            }
            "reimbursement" => to_value(reimbursement(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(InvoiceAssistant);

/// 常见税率（%）
const RATES: [f64; 6] = [13.0, 9.0, 6.0, 3.0, 1.0, 0.0];

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitArgs {
    /// 金额（元）
    pub amount: f64,
    /// 税率（%，如 13）
    pub rate: f64,
    /// 输入金额是含税（默认）还是不含税
    #[serde(default)]
    pub tax_included: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitOutput {
    pub rate: f64,
    pub gross: f64,
    pub net: f64,
    pub tax: f64,
}

pub fn tax_split(args: SplitArgs) -> PluginResult<SplitOutput> {
    if !args.amount.is_finite() || args.amount < 0.0 {
        return Err(PluginError::new("invoice.bad_amount").with("amount", args.amount));
    }
    if !RATES.contains(&args.rate) {
        return Err(PluginError::new("invoice.bad_rate").with("rate", args.rate));
    }
    let included = args.tax_included.unwrap_or(true);
    let factor = 1.0 + args.rate / 100.0;
    let (gross, net, tax) = if included {
        let net = args.amount / factor;
        (args.amount, net, args.amount - net)
    } else {
        let gross = args.amount * factor;
        (gross, args.amount, gross - args.amount)
    };
    Ok(SplitOutput {
        rate: args.rate,
        gross: round2(gross),
        net: round2(net),
        tax: round2(tax),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UppercaseArgs {
    /// 金额（元，最多两位小数）
    pub amount: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UppercaseOutput {
    /// 中文大写金额（如 壹仟贰佰叁拾肆元伍角陆分整）
    pub text: String,
    /// 合法性：金额在 0–万亿且最多两位小数
    pub valid: bool,
}

const DIGITS: [&str; 10] = ["零", "壹", "贰", "叁", "肆", "伍", "陆", "柒", "捌", "玖"];
const UNITS: [&str; 4] = ["", "拾", "佰", "仟"];
const GROUPS: [&str; 6] = ["", "万", "亿", "万亿", "亿亿", "万亿亿"];

/// 阿拉伯数字金额 → 中文大写（财务规范，含角分与「整」）
pub fn uppercase(amount: f64) -> PluginResult<UppercaseOutput> {
    // 两位小数判定不能直接用 (amount*100).fract()——f64 在大金额时会有 1e-7 量级的
    // 缩放误差（1234567.89 × 100 = 123456788.99999999），改用与取整结果的距离判断
    let cents_f = amount * 100.0;
    let valid = amount.is_finite()
        && (0.0..=1e16).contains(&amount)
        && (cents_f - cents_f.round()).abs() < 1e-6;
    if !valid {
        return Ok(UppercaseOutput {
            text: String::new(),
            valid: false,
        });
    }
    let cents = (amount * 100.0).round() as i128;
    if cents == 0 {
        return Ok(UppercaseOutput {
            text: "零元整".into(),
            valid: true,
        });
    }
    let yuan = cents / 100;
    let jiao = ((cents % 100) / 10) as usize;
    let fen = (cents % 10) as usize;

    let mut text = String::new();
    if yuan > 0 {
        text.push_str(&convert_integer(yuan));
        text.push('元');
    }
    match (jiao, fen) {
        (0, 0) => {
            if yuan > 0 {
                text.push('整');
            }
        }
        (j, 0) => {
            text.push_str(DIGITS[j]);
            text.push_str("角整");
        }
        (0, f) => {
            if yuan > 0 {
                text.push('零');
            }
            text.push_str(DIGITS[f]);
            text.push('分');
        }
        (j, f) => {
            text.push_str(DIGITS[j]);
            text.push('角');
            text.push_str(DIGITS[f]);
            text.push('分');
        }
    }
    Ok(UppercaseOutput { text, valid: true })
}

/// 整数部分按 4 位分组转换，处理零的读法（组间零与组内前导零）
fn convert_integer(value: i128) -> String {
    let mut groups: Vec<i128> = Vec::new();
    let mut rest = value;
    while rest > 0 {
        groups.push(rest % 10_000);
        rest /= 10_000;
    }
    let mut out = String::new();
    let mut need_zero = false;
    for gi in (0..groups.len()).rev() {
        let group = groups[gi];
        if group == 0 {
            // 整组为 0：只要低位还有非零组，就要补一个零
            if groups[..gi].iter().any(|&x| x > 0) {
                need_zero = true;
            }
            continue;
        }
        if need_zero {
            out.push('零');
            need_zero = false;
        }
        let digits = section_digits(group as usize);
        // 本组不是最高组且组内千位缺省时，高位补零（如 壹亿零壹万）
        if gi + 1 < groups.len() && digits[0] == 0 {
            out.push('零');
        }
        let mut section = String::new();
        let mut last_zero = false;
        for (position, digit) in digits.iter().enumerate() {
            if *digit == 0 {
                last_zero = true;
            } else {
                if last_zero && !section.is_empty() {
                    section.push('零');
                }
                last_zero = false;
                section.push_str(DIGITS[*digit]);
                section.push_str(UNITS[3 - position]);
            }
        }
        out.push_str(&section);
        out.push_str(GROUPS[gi]);
    }
    if out.is_empty() {
        out.push_str(DIGITS[0]);
    }
    out
}

/// 4 位组的各位数字（千位在先）
fn section_digits(group: usize) -> [usize; 4] {
    let value = group as u32;
    [
        (value / 1000) as usize,
        (value / 100 % 10) as usize,
        (value / 10 % 10) as usize,
        (value % 10) as usize,
    ]
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemArgs {
    pub name: String,
    /// 含税金额（元）
    pub amount: f64,
    /// 税率（%）
    pub rate: f64,
    #[serde(default)]
    pub category: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReimbursementArgs {
    pub title: String,
    pub person: String,
    pub items: Vec<ItemArgs>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReimbursementOutput {
    pub markdown: String,
    pub total: f64,
    pub tax_total: f64,
    pub count: usize,
}

/// 生成报销清单 Markdown（含价税分离明细与合计、大写金额）
pub fn reimbursement(args: ReimbursementArgs) -> PluginResult<ReimbursementOutput> {
    if args.items.is_empty() {
        return Err(PluginError::new("invoice.empty_items"));
    }
    let mut rows = Vec::new();
    let mut total = 0.0f64;
    let mut tax_total = 0.0f64;
    for (index, item) in args.items.iter().enumerate() {
        if item.name.trim().is_empty() {
            return Err(PluginError::new("invoice.bad_item").with("index", index as u64));
        }
        let split = tax_split(SplitArgs {
            amount: item.amount,
            rate: item.rate,
            tax_included: Some(true),
        })?;
        total += split.gross;
        tax_total += split.tax;
        let category = item.category.clone().unwrap_or_else(|| "-".into());
        rows.push(format!(
            "| {} | {} | {} | {:.2} | {:.2} | {:.2} |",
            index + 1,
            item.name.trim(),
            category,
            split.gross,
            split.tax,
            split.net
        ));
    }
    let upper = uppercase(round2(total))?.text;
    let (t, p) = (args.title.trim(), args.person.trim());
    let markdown = format!(
        "# {t}\n\n报销人：{p}\n\n| # | 项目 | 类别 | 含税金额 | 税额 | 不含税金额 |\n|---|------|------|----------|------|------------|\n{}\n\n合计：{:.2} 元（税额 {:.2} 元）\n\n大写：{upper}\n",
        rows.join("\n"),
        round2(total),
        round2(tax_total)
    );
    Ok(ReimbursementOutput {
        markdown,
        total: round2(total),
        tax_total: round2(tax_total),
        count: args.items.len(),
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
