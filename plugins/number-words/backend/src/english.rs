//! 英文读法（美式短级差，不加 and）与支票写法。

use tf_plugin_api::{PluginError, PluginResult};

use crate::number::Number;

const ONES: [&str; 20] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];
const TENS: [&str; 10] = [
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];
const SCALES: [&str; 12] = [
    "",
    "thousand",
    "million",
    "billion",
    "trillion",
    "quadrillion",
    "quintillion",
    "sextillion",
    "septillion",
    "octillion",
    "nonillion",
    "decillion",
];

fn below_thousand(n: u32) -> String {
    let mut parts = Vec::new();
    if n >= 100 {
        parts.push(format!("{} hundred", ONES[(n / 100) as usize]));
    }
    let rest = n % 100;
    if rest >= 20 {
        let tens = TENS[(rest / 10) as usize];
        parts.push(if rest.is_multiple_of(10) {
            tens.to_owned()
        } else {
            format!("{tens}-{}", ONES[(rest % 10) as usize])
        });
    } else if rest > 0 {
        parts.push(ONES[rest as usize].to_owned());
    }
    parts.join(" ")
}

fn integer(int: &str) -> PluginResult<String> {
    if int == "0" {
        return Ok("zero".to_owned());
    }
    let groups: Vec<u32> = {
        let bytes = int.as_bytes();
        let mut out = Vec::new();
        let mut end = bytes.len();
        while end > 0 {
            let start = end.saturating_sub(3);
            out.push(
                std::str::from_utf8(&bytes[start..end])
                    .unwrap()
                    .parse()
                    .unwrap(),
            );
            end = start;
        }
        out
    };
    if groups.len() > SCALES.len() {
        return Err(PluginError::new("num.too_large_english"));
    }
    let words: Vec<String> = groups
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, g)| **g > 0)
        .map(|(i, g)| {
            let words = below_thousand(*g);
            if SCALES[i].is_empty() {
                words
            } else {
                format!("{words} {}", SCALES[i])
            }
        })
        .collect();
    Ok(words.join(" "))
}

/// one thousand two hundred thirty-four point five six
pub fn words(n: &Number) -> PluginResult<String> {
    let mut out = String::new();
    if n.negative {
        out.push_str("minus ");
    }
    out.push_str(&integer(&n.int)?);
    if !n.frac.is_empty() {
        out.push_str(" point");
        for b in n.frac.bytes() {
            out.push(' ');
            out.push_str(ONES[(b - b'0') as usize]);
        }
    }
    Ok(out)
}

fn title(text: &str) -> String {
    text.split(' ')
        .map(|word| {
            word.split('-')
                .map(|part| {
                    let mut chars = part.chars();
                    match chars.next() {
                        Some(first) => first.to_uppercase().chain(chars).collect(),
                        None => String::new(),
                    }
                })
                .collect::<Vec<String>>()
                .join("-")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// 支票写法：One Thousand Two Hundred Thirty-Four and 56/100
pub fn check(n: &Number) -> PluginResult<(String, bool)> {
    let rounded = n.round(2);
    let cents = format!("{:0<2}", rounded.frac);
    let dollars = title(&integer(&rounded.int)?);
    let sign = if rounded.negative { "Minus " } else { "" };
    Ok((format!("{sign}{dollars} and {cents}/100"), rounded != *n))
}

#[cfg(test)]
#[path = "english_test.rs"]
mod tests;
