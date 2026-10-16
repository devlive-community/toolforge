//! 卡组织号段与长度规则、Luhn 校验。只按公开号段规则识别品牌，不代表真实发卡行或卡片状态。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Brand {
    Visa,
    Mastercard,
    Amex,
    Discover,
    Jcb,
    Unionpay,
    Diners,
}

pub const ALL: [Brand; 7] = [
    Brand::Visa,
    Brand::Mastercard,
    Brand::Amex,
    Brand::Discover,
    Brand::Jcb,
    Brand::Unionpay,
    Brand::Diners,
];

/// 号段：前缀区间（含两端，位数相同）
struct Range(u32, u32);

impl Brand {
    fn ranges(self) -> &'static [Range] {
        match self {
            Brand::Visa => &[Range(4, 4)],
            Brand::Mastercard => &[Range(51, 55), Range(2221, 2720)],
            Brand::Amex => &[Range(34, 34), Range(37, 37)],
            Brand::Discover => &[Range(6011, 6011), Range(644, 649), Range(65, 65)],
            Brand::Jcb => &[Range(3528, 3589)],
            Brand::Unionpay => &[Range(62, 62)],
            Brand::Diners => &[Range(36, 36), Range(38, 38), Range(300, 305)],
        }
    }

    /// 允许的卡号长度
    pub fn lengths(self) -> &'static [usize] {
        match self {
            Brand::Visa => &[13, 16, 19],
            Brand::Mastercard => &[16],
            Brand::Amex => &[15],
            Brand::Discover => &[16, 19],
            Brand::Jcb => &[16, 17, 18, 19],
            Brand::Unionpay => &[16, 17, 18, 19],
            Brand::Diners => &[14, 16, 19],
        }
    }

    /// 生成时使用的长度
    pub fn default_length(self) -> usize {
        match self {
            Brand::Amex => 15,
            Brand::Diners => 14,
            _ => 16,
        }
    }

    pub fn cvc_length(self) -> usize {
        if self == Brand::Amex { 4 } else { 3 }
    }

    /// 号段起始的数字串（生成时随机选一个区间内的值作为前缀）
    pub fn prefixes(self) -> Vec<String> {
        self.ranges()
            .iter()
            .flat_map(|r| (r.0..=r.1).map(|p| p.to_string()))
            .collect()
    }

    fn matches(self, digits: &str) -> bool {
        self.ranges().iter().any(|r| {
            let width = r.0.to_string().len();
            digits.len() >= width
                && digits[..width]
                    .parse::<u32>()
                    .is_ok_and(|p| (r.0..=r.1).contains(&p))
        })
    }
}

/// 按号段识别品牌（更长、更具体的号段优先，如 Discover 6011 先于 UnionPay 62 之外的规则）
pub fn detect(digits: &str) -> Option<Brand> {
    ALL.into_iter().find(|b| b.matches(digits))
}

pub fn luhn_valid(digits: &str) -> bool {
    !digits.is_empty() && luhn_sum(digits).is_multiple_of(10)
}

fn luhn_sum(digits: &str) -> u32 {
    digits
        .bytes()
        .rev()
        .enumerate()
        .map(|(i, b)| {
            let d = (b - b'0') as u32;
            if i % 2 == 1 {
                let x = d * 2;
                if x > 9 { x - 9 } else { x }
            } else {
                d
            }
        })
        .sum()
}

/// 为不含校验位的数字串计算 Luhn 校验位
pub fn check_digit(body: &str) -> u8 {
    let sum = luhn_sum(&format!("{body}0"));
    ((10 - sum % 10) % 10) as u8
}

#[cfg(test)]
#[path = "brand_test.rs"]
mod tests;
