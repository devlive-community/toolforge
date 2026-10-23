//! 中文数字：小写（一千二百）、大写（壹仟贰佰）、人民币金额大写，以及解析中文数字。

use tf_plugin_api::{PluginError, PluginResult};

use crate::number::Number;

const LOWER: [char; 10] = ['零', '一', '二', '三', '四', '五', '六', '七', '八', '九'];
const UPPER: [char; 10] = ['零', '壹', '贰', '叁', '肆', '伍', '陆', '柒', '捌', '玖'];
const LOWER_UNITS: [&str; 4] = ["", "十", "百", "千"];
const UPPER_UNITS: [&str; 4] = ["", "拾", "佰", "仟"];
/// 四位一节的节名（万亿之后不再扩展）
const SECTIONS: [&str; 4] = ["", "万", "亿", "万亿"];
/// 中文写法支持的整数位数（小于 10^16）
const MAX_CHINESE_DIGITS: usize = 16;

#[derive(Clone, Copy, PartialEq)]
pub enum Case {
    Lower,
    Upper,
}

fn digits(case: Case) -> &'static [char; 10] {
    match case {
        Case::Lower => &LOWER,
        Case::Upper => &UPPER,
    }
}

/// 整数部分的中文写法；leading_ten 为 true 时 10–19 写成“十几”而不是“一十几”
fn integer(int: &str, case: Case, leading_ten: bool) -> PluginResult<String> {
    if int.len() > MAX_CHINESE_DIGITS {
        return Err(PluginError::new("num.too_large_chinese"));
    }
    if int == "0" {
        return Ok(digits(case)[0].to_string());
    }
    let units = match case {
        Case::Lower => &LOWER_UNITS,
        Case::Upper => &UPPER_UNITS,
    };
    let nums: Vec<u8> = int.bytes().map(|b| b - b'0').collect();
    let sections = nums.len().div_ceil(4);
    let mut out = String::new();
    // 上一节之后是否需要补“零”（本节高位为 0 或前面有整节为 0）
    let mut pending_zero = false;
    for s in 0..sections {
        let end = nums.len() - (sections - 1 - s) * 4;
        let start = end.saturating_sub(4);
        let section = &nums[start..end];
        let name = SECTIONS[sections - 1 - s];
        if section.iter().all(|&d| d == 0) {
            pending_zero = !out.is_empty();
            continue;
        }
        let offset = 4 - section.len();
        let mut zero = pending_zero || (!out.is_empty() && section.len() == 4 && section[0] == 0);
        for (i, &d) in section.iter().enumerate() {
            let position = 3 - (i + offset);
            if d == 0 {
                zero = !out.is_empty();
                continue;
            }
            if zero {
                out.push(digits(case)[0]);
                zero = false;
            }
            let ten_only =
                leading_ten && case == Case::Lower && out.is_empty() && d == 1 && position == 1;
            if !ten_only {
                out.push(digits(case)[d as usize]);
            }
            out.push_str(units[position]);
        }
        out.push_str(name);
        pending_zero = false;
    }
    Ok(out)
}

fn fraction(frac: &str, case: Case) -> String {
    frac.bytes()
        .map(|b| digits(case)[(b - b'0') as usize])
        .collect()
}

/// 一千二百三十四点五六 / 壹仟贰佰叁拾肆点伍陆
pub fn numerals(n: &Number, case: Case) -> PluginResult<String> {
    let mut out = String::new();
    if n.negative {
        out.push('负');
    }
    out.push_str(&integer(&n.int, case, true)?);
    if !n.frac.is_empty() {
        out.push('点');
        out.push_str(&fraction(&n.frac, case));
    }
    Ok(out)
}

/// 人民币大写：四舍五入到分；元后无角分写“整”，有角无分不写“整”
pub fn rmb(n: &Number) -> PluginResult<(String, bool)> {
    let rounded = n.round(2);
    let changed = rounded != *n;
    let frac = format!("{:0<2}", rounded.frac);
    let (jiao, fen) = (frac.as_bytes()[0] - b'0', frac.as_bytes()[1] - b'0');
    let mut out = String::new();
    if rounded.negative {
        out.push('负');
    }
    let has_yuan = rounded.int != "0";
    if has_yuan {
        out.push_str(&integer(&rounded.int, Case::Upper, false)?);
        out.push('元');
    }
    match (jiao, fen) {
        (0, 0) if has_yuan => out.push('整'),
        (0, 0) => out.push_str("零元整"),
        _ => {
            if jiao > 0 {
                out.push(UPPER[jiao as usize]);
                out.push('角');
            } else if has_yuan {
                out.push('零');
            }
            if fen > 0 {
                out.push(UPPER[fen as usize]);
                out.push('分');
            }
        }
    }
    Ok((out, changed))
}

fn digit_value(c: char) -> Option<u8> {
    match c {
        '零' | '〇' | '0' => Some(0),
        '一' | '壹' | '幺' => Some(1),
        '二' | '贰' | '两' | '貳' => Some(2),
        '三' | '叁' | '參' => Some(3),
        '四' | '肆' => Some(4),
        '五' | '伍' => Some(5),
        '六' | '陆' | '陸' => Some(6),
        '七' | '柒' => Some(7),
        '八' | '捌' => Some(8),
        '九' | '玖' => Some(9),
        _ => None,
    }
}

fn small_unit(c: char) -> Option<u128> {
    match c {
        '十' | '拾' => Some(10),
        '百' | '佰' => Some(100),
        '千' | '仟' => Some(1000),
        _ => None,
    }
}

/// 解析中文整数（支持 万、亿 及“十二”“两千”“一万零五”等写法）
fn parse_integer(text: &str) -> Option<u128> {
    if text.is_empty() {
        return Some(0);
    }
    // 全部是数字字符时按位读（如 二〇二四）
    if text.chars().all(|c| digit_value(c).is_some()) && text.chars().count() > 1 {
        return text.chars().try_fold(0u128, |acc, c| {
            acc.checked_mul(10)?.checked_add(digit_value(c)? as u128)
        });
    }
    // total：亿以上；mid：亿以下、万以上；section：万以下；number：最近读到的数字
    let (mut total, mut mid, mut section, mut number) = (0u128, 0u128, 0u128, None::<u128>);
    for c in text.chars() {
        if let Some(d) = digit_value(c) {
            number = Some(d as u128);
        } else if let Some(unit) = small_unit(c) {
            // “十二”省略了“一”
            section = section.checked_add(number.take().unwrap_or(1).checked_mul(unit)?)?;
        } else if c == '万' || c == '萬' {
            let part = section + number.take().unwrap_or(0);
            mid = (mid + if part == 0 && mid == 0 { 1 } else { part }).checked_mul(10_000)?;
            section = 0;
        } else if c == '亿' || c == '億' {
            let part = mid + section + number.take().unwrap_or(0);
            total = (total + if part == 0 && total == 0 { 1 } else { part })
                .checked_mul(100_000_000)?;
            mid = 0;
            section = 0;
        } else {
            return None;
        }
    }
    total
        .checked_add(mid)?
        .checked_add(section)?
        .checked_add(number.unwrap_or(0))
}

/// 解析中文数字或人民币大写金额；不是中文数字时返回 None
pub fn parse(input: &str) -> Option<PluginResult<(Number, bool)>> {
    let text: String = input
        .trim()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let (negative, text) = match text.strip_prefix('负').or_else(|| text.strip_prefix('-')) {
        Some(rest) => (true, rest.to_owned()),
        None => (false, text),
    };
    let text = text.trim_start_matches("人民币").to_owned();
    if text.is_empty()
        || !text.chars().any(|c| {
            digit_value(c).is_some() || small_unit(c).is_some() || "万亿元角分".contains(c)
        })
    {
        return None;
    }
    // 金额：xx元x角x分[整]
    if text.contains(['元', '角', '分', '圆']) {
        let body = text.trim_end_matches(['整', '正']);
        let (yuan, rest) = match body.split_once(['元', '圆']) {
            Some((y, r)) => (y, r),
            None => ("", body),
        };
        let (jiao, fen) = match rest.split_once('角') {
            Some((j, f)) => (j.trim_start_matches('零'), f),
            None => ("", rest),
        };
        let fen = fen.trim_start_matches('零').trim_end_matches('分');
        let one = |s: &str| -> Option<u8> {
            match s.chars().count() {
                0 => Some(0),
                1 => digit_value(s.chars().next()?),
                _ => None,
            }
        };
        let int = parse_integer(yuan.trim_end_matches('零'))?;
        let (j, f) = (one(jiao)?, one(fen)?);
        return Some(
            Number::new(negative, &int.to_string(), &format!("{j}{f}")).map(|n| (n, true)),
        );
    }
    let (int_part, frac_part) = text.split_once('点').unwrap_or((&text, ""));
    let int = parse_integer(int_part)?;
    let frac: Option<String> = frac_part
        .chars()
        .map(|c| digit_value(c).map(|d| (b'0' + d) as char))
        .collect();
    Some(Number::new(negative, &int.to_string(), &frac?).map(|n| (n, false)))
}

#[cfg(test)]
#[path = "chinese_test.rs"]
mod tests;
