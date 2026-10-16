//! GB 32100-2015《法人和其他组织统一社会信用代码编码规则》及第 1 号修改单。

use serde::Serialize;

pub const RULES: &str = "GB 32100-2015 + Amendment No.1";
/// 代码字符集：阿拉伯数字与不含 I、O、Z、S、V 的大写字母，下标即字符的数值
pub const CHARSET: &str = "0123456789ABCDEFGHJKLMNPQRTUWXY";
const WEIGHTS: [u32; 17] = [
    1, 3, 9, 27, 19, 26, 16, 17, 20, 29, 25, 13, 8, 24, 10, 30, 28,
];
/// GB 11714 组织机构代码（第 9–17 位）的校验权重
const ORG_WEIGHTS: [u32; 8] = [3, 7, 9, 10, 5, 8, 4, 2];

/// (机构类别码, 名称键)
pub type Category = (char, &'static str);

/// 登记管理部门码 → (名称键, 机构类别)
pub const DEPARTMENTS: &[(char, &str, &[Category])] = &[
    (
        '1',
        "establishment",
        &[
            ('1', "organ"),
            ('2', "institution"),
            ('3', "mass_organization"),
            ('9', "other"),
        ],
    ),
    (
        '2',
        "foreign_affairs",
        &[('1', "foreign_news_agency"), ('9', "other")],
    ),
    (
        '3',
        "justice",
        &[
            ('1', "law_firm"),
            ('2', "notary"),
            ('3', "legal_service"),
            ('4', "forensic"),
            ('5', "arbitration"),
            ('9', "other"),
        ],
    ),
    (
        '4',
        "culture",
        &[('1', "foreign_culture_center"), ('9', "other")],
    ),
    (
        '5',
        "civil_affairs",
        &[
            ('1', "social_group"),
            ('2', "private_non_enterprise"),
            ('3', "foundation"),
            ('9', "other"),
        ],
    ),
    (
        '6',
        "tourism",
        &[
            ('1', "foreign_tourism_office"),
            ('2', "hmt_tourism_office"),
            ('9', "other"),
        ],
    ),
    (
        '7',
        "religion",
        &[
            ('1', "religious_venue"),
            ('2', "religious_school"),
            ('9', "other"),
        ],
    ),
    (
        '8',
        "trade_union",
        &[('1', "grassroots_union"), ('9', "other")],
    ),
    (
        '9',
        "industry_commerce",
        &[
            ('1', "enterprise"),
            ('2', "individual_business"),
            ('3', "farmer_cooperative"),
        ],
    ),
    (
        'A',
        "military",
        &[('1', "military_institution"), ('9', "other")],
    ),
    (
        'N',
        "agriculture",
        &[
            ('1', "group_collective"),
            ('2', "village_collective"),
            ('3', "township_collective"),
            ('9', "other"),
        ],
    ),
    ('Y', "other", &[('1', "other")]),
];

/// 省级行政区划（GB/T 2260 前两位）；更细的区划码只显示代码，需人工核对
pub const PROVINCES: &[(&str, &str)] = &[
    ("10", "national"),
    ("11", "beijing"),
    ("12", "tianjin"),
    ("13", "hebei"),
    ("14", "shanxi"),
    ("15", "inner_mongolia"),
    ("21", "liaoning"),
    ("22", "jilin"),
    ("23", "heilongjiang"),
    ("31", "shanghai"),
    ("32", "jiangsu"),
    ("33", "zhejiang"),
    ("34", "anhui"),
    ("35", "fujian"),
    ("36", "jiangxi"),
    ("37", "shandong"),
    ("41", "henan"),
    ("42", "hubei"),
    ("43", "hunan"),
    ("44", "guangdong"),
    ("45", "guangxi"),
    ("46", "hainan"),
    ("50", "chongqing"),
    ("51", "sichuan"),
    ("52", "guizhou"),
    ("53", "yunnan"),
    ("54", "tibet"),
    ("61", "shaanxi"),
    ("62", "gansu"),
    ("63", "qinghai"),
    ("64", "ningxia"),
    ("65", "xinjiang"),
    ("71", "taiwan"),
    ("81", "hong_kong"),
    ("82", "macao"),
];

fn value(c: char) -> Option<u32> {
    CHARSET.find(c).map(|i| i as u32)
}

/// 由前 17 位计算校验码；含非法字符时返回 None
pub fn check_char(first17: &str) -> Option<char> {
    let mut sum = 0;
    for (c, w) in first17.chars().zip(WEIGHTS) {
        sum += value(c)? * w;
    }
    let check = (31 - sum % 31) % 31;
    CHARSET.chars().nth(check as usize)
}

/// GB 11714 组织机构代码的校验码（数字与全部大写字母参与计算）
pub fn org_check_char(first8: &str) -> Option<char> {
    let mut sum = 0;
    for (c, w) in first8.chars().zip(ORG_WEIGHTS) {
        let v = c.to_digit(36)?;
        sum += v * w;
    }
    Some(match 11 - sum % 11 {
        10 => 'X',
        11 => '0',
        n => char::from_digit(n, 10)?,
    })
}

/// 输入的规范化：去掉首尾与中间空白、全角转半角、小写转大写；记录每一种修改
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Normalized {
    pub value: String,
    pub trimmed: bool,
    pub removed_spaces: bool,
    pub uppercased: bool,
    pub fullwidth: bool,
}

pub fn normalize(raw: &str) -> Normalized {
    let trimmed = raw.trim();
    let mut out = Normalized {
        trimmed: trimmed.len() != raw.len(),
        ..Normalized::default()
    };
    for c in trimmed.chars() {
        if c.is_whitespace() {
            out.removed_spaces = true;
            continue;
        }
        let half = match c {
            '０'..='９' | 'Ａ'..='Ｚ' | 'ａ'..='ｚ' => {
                out.fullwidth = true;
                char::from_u32(c as u32 - 0xFEE0).unwrap_or(c)
            }
            c => c,
        };
        if half.is_ascii_lowercase() {
            out.uppercased = true;
        }
        out.value.push(half.to_ascii_uppercase());
    }
    out
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Problem {
    /// 长度不是 18 位
    Length { found: usize },
    /// 不在字符集中的字符（位置 1 起始）
    Char { position: usize, found: String },
    /// 校验位不一致
    Check { expected: String, found: String },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub code: String,
    /// 码表中的名称键（前端翻译）；码表无法确认时为 None
    pub name: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Fields {
    pub department: Field,
    pub category: Field,
    /// 行政区划码与其省级名称
    pub region: Field,
    pub subject: String,
    /// 主体标识码按 GB 11714 计算的校验码是否一致（不影响整体结论）
    pub org_check_ok: bool,
    pub org_check_expected: Option<String>,
    pub check: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    /// 长度、字符、校验位都符合，且码表能确认部门与类别
    Valid,
    /// 结构正确，但登记管理部门或机构类别不在内置码表中，需要人工核对
    Unconfirmed,
    Invalid,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub status: Status,
    pub problems: Vec<Problem>,
    pub fields: Option<Fields>,
}

pub fn check(code: &str) -> Report {
    let chars: Vec<char> = code.chars().collect();
    let mut problems = Vec::new();
    if chars.len() != 18 {
        problems.push(Problem::Length { found: chars.len() });
    }
    for (index, c) in chars.iter().enumerate() {
        if value(*c).is_none() {
            problems.push(Problem::Char {
                position: index + 1,
                found: c.to_string(),
            });
        }
    }
    if !problems.is_empty() {
        return Report {
            status: Status::Invalid,
            problems,
            fields: None,
        };
    }
    let first17: String = chars[..17].iter().collect();
    let expected = check_char(&first17).expect("validated charset");
    if expected != chars[17] {
        problems.push(Problem::Check {
            expected: expected.to_string(),
            found: chars[17].to_string(),
        });
    }
    let department = DEPARTMENTS.iter().find(|(c, ..)| *c == chars[0]);
    let category = department.and_then(|(_, _, cats)| cats.iter().find(|(c, _)| *c == chars[1]));
    let region: String = chars[2..8].iter().collect();
    let subject: String = chars[8..17].iter().collect();
    let org_expected = org_check_char(&subject[..8]);
    let fields = Fields {
        department: Field {
            code: chars[0].to_string(),
            name: department.map(|(_, name, _)| *name),
        },
        category: Field {
            code: chars[1].to_string(),
            name: category.map(|(_, name)| *name),
        },
        region: Field {
            name: PROVINCES
                .iter()
                .find(|(code, _)| region.starts_with(code))
                .map(|(_, name)| *name),
            code: region,
        },
        org_check_ok: org_expected == subject.chars().last(),
        org_check_expected: org_expected.map(String::from),
        subject,
        check: chars[17].to_string(),
    };
    let status = if !problems.is_empty() {
        Status::Invalid
    } else if department.is_none() || category.is_none() {
        Status::Unconfirmed
    } else {
        Status::Valid
    };
    Report {
        status,
        problems,
        fields: Some(fields),
    }
}

#[cfg(test)]
#[path = "code_test.rs"]
mod tests;
