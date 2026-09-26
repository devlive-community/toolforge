//! 按字段定义生成数据。同一行的姓名、邮箱、性别、生日、年龄、身份证号、地址彼此一致；
//! 使用 ChaCha8 与固定种子，同样的配置与种子总是生成同样的数据。

use jiff::civil::Date;
use rand::seq::IndexedRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tf_plugin_api::{PluginError, PluginResult};

use crate::data::*;

pub const MAX_ROWS: usize = 100_000;
pub const MAX_FIELDS: usize = 50;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Locale {
    ZhCn,
    EnUs,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Id,
    Uuid,
    Name,
    FirstName,
    LastName,
    Username,
    Email,
    Phone,
    IdCard,
    Gender,
    Age,
    Birthday,
    Date,
    Datetime,
    Timestamp,
    Integer,
    Float,
    Boolean,
    Enum,
    Province,
    City,
    Address,
    Zip,
    Company,
    Job,
    Url,
    Ip,
    Ipv6,
    Mac,
    Color,
    Password,
    Word,
    Sentence,
    Paragraph,
    Constant,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub name: String,
    pub kind: Kind,
    /// 数值 / 年龄的下限（序号的起始值）
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    /// 小数位数
    #[serde(default)]
    pub decimals: Option<u32>,
    /// 日期范围（YYYY-MM-DD）
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    /// 枚举选项（逗号分隔）或常量值
    #[serde(default)]
    pub options: Option<String>,
    /// 为空的概率（0–100）
    #[serde(default)]
    pub null_percent: u8,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Schema {
    pub locale: Locale,
    pub rows: usize,
    pub fields: Vec<Field>,
    /// 为空时随机生成并在结果中返回
    #[serde(default)]
    pub seed: Option<u64>,
}

/// 一行中共享的「人物」信息
struct Person {
    name: String,
    first: String,
    last: String,
    /// 用于用户名与邮箱的拼音 / 英文小写
    handle: String,
    male: bool,
    birthday: Date,
    region: usize,
    street_no: u32,
}

fn invalid(message: &str, field: &str) -> PluginError {
    PluginError::new(message).with("field", field)
}

pub fn validate(schema: &Schema) -> PluginResult<()> {
    if schema.rows == 0 || schema.rows > MAX_ROWS {
        return Err(PluginError::new("mock.invalid_rows").with("max", MAX_ROWS));
    }
    if schema.fields.is_empty() {
        return Err(PluginError::new("mock.no_fields"));
    }
    if schema.fields.len() > MAX_FIELDS {
        return Err(PluginError::new("mock.too_many_fields").with("max", MAX_FIELDS));
    }
    let mut names = std::collections::HashSet::new();
    for field in &schema.fields {
        let name = field.name.trim();
        if name.is_empty() {
            return Err(PluginError::new("mock.empty_name"));
        }
        if !names.insert(name) {
            return Err(invalid("mock.duplicate_name", name));
        }
        if let (Some(min), Some(max)) = (field.min, field.max)
            && min > max
        {
            return Err(invalid("mock.invalid_range", name));
        }
        if field.kind == Kind::Enum && options(field).is_empty() {
            return Err(invalid("mock.no_options", name));
        }
        date_range(field, (2020, 2026))?;
    }
    Ok(())
}

fn options(field: &Field) -> Vec<&str> {
    field
        .options
        .as_deref()
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|o| !o.is_empty())
        .collect()
}

fn parse_date(text: &str, field: &str) -> PluginResult<Date> {
    text.trim()
        .parse::<Date>()
        .map_err(|_| invalid("mock.invalid_date", field).with("value", text))
}

/// 日期范围，缺省为给定的年份区间
fn date_range(field: &Field, years: (i16, i16)) -> PluginResult<(Date, Date)> {
    let from = match field.from.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(text) => parse_date(text, &field.name)?,
        None => Date::new(years.0, 1, 1).unwrap(),
    };
    let to = match field.to.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(text) => parse_date(text, &field.name)?,
        None => Date::new(years.1, 12, 31).unwrap(),
    };
    if from > to {
        return Err(invalid("mock.invalid_range", &field.name));
    }
    Ok((from, to))
}

fn random_date(rng: &mut ChaCha8Rng, from: Date, to: Date) -> Date {
    let days = (to - from).get_days().max(0);
    from.checked_add(jiff::Span::new().days(rng.random_range(0..=days)))
        .unwrap_or(from)
}

fn pick<'a, T>(rng: &mut ChaCha8Rng, list: &'a [T]) -> &'a T {
    list.choose(rng).expect("non-empty data list")
}

/// GB 11643-1999 校验码
pub fn id_checksum(first17: &str) -> char {
    const WEIGHTS: [u32; 17] = [7, 9, 10, 5, 8, 4, 2, 1, 6, 3, 7, 9, 10, 5, 8, 4, 2];
    const CODES: [char; 11] = ['1', '0', 'X', '9', '8', '7', '6', '5', '4', '3', '2'];
    let sum: u32 = first17
        .chars()
        .zip(WEIGHTS)
        .map(|(c, w)| c.to_digit(10).unwrap_or(0) * w)
        .sum();
    CODES[(sum % 11) as usize]
}

fn person(rng: &mut ChaCha8Rng, locale: Locale, age: (u32, u32), today: Date) -> Person {
    let male = rng.random_bool(0.5);
    let years = rng.random_range(age.0..=age.1.max(age.0));
    let latest = today
        .checked_sub(jiff::Span::new().years(years))
        .unwrap_or(today);
    let earliest = latest
        .checked_sub(jiff::Span::new().years(1))
        .and_then(|d| d.checked_add(jiff::Span::new().days(1)))
        .unwrap_or(latest);
    let birthday = random_date(rng, earliest, latest);
    match locale {
        Locale::ZhCn => {
            let (last, last_py) = *pick(rng, ZH_SURNAMES);
            let (first, first_py) = *pick(rng, if male { ZH_MALE } else { ZH_FEMALE });
            Person {
                name: format!("{last}{first}"),
                first: first.into(),
                last: last.into(),
                handle: format!("{last_py}{first_py}"),
                male,
                birthday,
                region: rng.random_range(0..ZH_REGIONS.len()),
                street_no: rng.random_range(1..=999),
            }
        }
        Locale::EnUs => {
            let first = *pick(rng, if male { EN_FIRST_MALE } else { EN_FIRST_FEMALE });
            let last = *pick(rng, EN_LAST);
            Person {
                name: format!("{first} {last}"),
                first: first.into(),
                last: last.into(),
                handle: format!("{}.{}", first.to_lowercase(), last.to_lowercase()),
                male,
                birthday,
                region: rng.random_range(0..EN_CITIES.len()),
                street_no: rng.random_range(1..=9999),
            }
        }
    }
}

fn age_of(birthday: Date, today: Date) -> i64 {
    let mut age = (today.year() - birthday.year()) as i64;
    if (today.month(), today.day()) < (birthday.month(), birthday.day()) {
        age -= 1;
    }
    age
}

fn sentence(rng: &mut ChaCha8Rng, locale: Locale) -> String {
    let count = rng.random_range(6..=14);
    match locale {
        Locale::ZhCn => {
            let words: String = (0..count).map(|_| *pick(rng, ZH_WORDS)).collect();
            format!("{words}。")
        }
        Locale::EnUs => {
            let words: Vec<&str> = (0..count).map(|_| *pick(rng, EN_WORDS)).collect();
            let text = words.join(" ");
            let mut chars = text.chars();
            let first = chars
                .next()
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_default();
            format!("{first}{}.", chars.as_str())
        }
    }
}

fn round(value: f64, decimals: u32) -> f64 {
    let factor = 10f64.powi(decimals as i32);
    (value * factor).round() / factor
}

fn uuid(rng: &mut ChaCha8Rng) -> String {
    let mut bytes: [u8; 16] = rng.random();
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

fn value(
    field: &Field,
    row: usize,
    p: &Person,
    locale: Locale,
    today: Date,
    rng: &mut ChaCha8Rng,
) -> Value {
    let zh = locale == Locale::ZhCn;
    let text = |s: String| Value::String(s);
    match field.kind {
        Kind::Id => Value::from(field.min.unwrap_or(1.0) as i64 + row as i64),
        Kind::Uuid => text(uuid(rng)),
        Kind::Name => text(p.name.clone()),
        Kind::FirstName => text(p.first.clone()),
        Kind::LastName => text(p.last.clone()),
        Kind::Username => text(format!(
            "{}{}",
            p.handle.replace('.', "_"),
            rng.random_range(1..=999)
        )),
        Kind::Email => {
            let domain = pick(
                rng,
                if zh {
                    ZH_EMAIL_DOMAINS
                } else {
                    EN_EMAIL_DOMAINS
                },
            );
            text(format!("{}{}@{domain}", p.handle, rng.random_range(1..=99)))
        }
        Kind::Phone => {
            if zh {
                let prefix = pick(rng, ZH_MOBILE_PREFIXES);
                text(format!(
                    "{prefix}{:08}",
                    rng.random_range(0..100_000_000u32)
                ))
            } else {
                let area = EN_CITIES[p.region].4;
                text(format!(
                    "({area}) {:03}-{:04}",
                    rng.random_range(200..1000u32),
                    rng.random_range(0..10_000u32)
                ))
            }
        }
        Kind::IdCard => {
            if zh {
                let region = ZH_REGIONS[p.region].code;
                let sequence = rng.random_range(0..500u32) * 2 + u32::from(p.male);
                let first17 = format!(
                    "{region}{:04}{:02}{:02}{sequence:03}",
                    p.birthday.year(),
                    p.birthday.month(),
                    p.birthday.day()
                );
                let check = id_checksum(&first17);
                text(format!("{first17}{check}"))
            } else {
                text(format!(
                    "{:03}-{:02}-{:04}",
                    rng.random_range(100..900u32),
                    rng.random_range(10..100u32),
                    rng.random_range(1000..10_000u32)
                ))
            }
        }
        Kind::Gender => text(
            match (zh, p.male) {
                (true, true) => "男",
                (true, false) => "女",
                (false, true) => "male",
                (false, false) => "female",
            }
            .into(),
        ),
        Kind::Age => Value::from(age_of(p.birthday, today)),
        Kind::Birthday => text(p.birthday.to_string()),
        Kind::Date => {
            let (from, to) =
                date_range(field, (today.year() - 3, today.year())).unwrap_or((today, today));
            text(random_date(rng, from, to).to_string())
        }
        Kind::Datetime | Kind::Timestamp => {
            let (from, to) =
                date_range(field, (today.year() - 3, today.year())).unwrap_or((today, today));
            let day = random_date(rng, from, to);
            let seconds = rng.random_range(0..86_400i64);
            let time = day
                .to_datetime(jiff::civil::Time::midnight())
                .checked_add(jiff::Span::new().seconds(seconds))
                .unwrap_or(day.to_datetime(jiff::civil::Time::midnight()));
            if field.kind == Kind::Timestamp {
                Value::from(
                    time.to_zoned(jiff::tz::TimeZone::UTC)
                        .map(|z| z.timestamp().as_second())
                        .unwrap_or_default(),
                )
            } else {
                text(time.strftime("%Y-%m-%d %H:%M:%S").to_string())
            }
        }
        Kind::Integer => {
            let (min, max) = (
                field.min.unwrap_or(0.0) as i64,
                field.max.unwrap_or(1000.0) as i64,
            );
            Value::from(rng.random_range(min..=max.max(min)))
        }
        Kind::Float => {
            let (min, max) = (field.min.unwrap_or(0.0), field.max.unwrap_or(1000.0));
            let raw = if max > min {
                rng.random_range(min..max)
            } else {
                min
            };
            serde_json::Number::from_f64(round(raw, field.decimals.unwrap_or(2).min(10)))
                .map(Value::Number)
                .unwrap_or(Value::Null)
        }
        Kind::Boolean => Value::Bool(rng.random_bool(0.5)),
        Kind::Enum => {
            let list = options(field);
            text((*pick(rng, &list)).to_owned())
        }
        Kind::Province => text(if zh {
            ZH_REGIONS[p.region].province.into()
        } else {
            EN_CITIES[p.region].1.into()
        }),
        Kind::City => text(if zh {
            ZH_REGIONS[p.region].city.into()
        } else {
            EN_CITIES[p.region].0.into()
        }),
        Kind::Address => {
            if zh {
                let region = &ZH_REGIONS[p.region];
                let city = if region.province == region.city {
                    ""
                } else {
                    region.city
                };
                let street = pick(rng, ZH_STREETS);
                let suffix = pick(rng, ZH_STREET_SUFFIXES);
                let community = pick(rng, ZH_COMMUNITIES);
                text(format!(
                    "{}{city}{}{street}{suffix}{}号{community}{}栋{}室",
                    region.province,
                    region.district,
                    p.street_no,
                    rng.random_range(1..=30),
                    rng.random_range(101..=2808)
                ))
            } else {
                let (city, _, state, zip, _) = EN_CITIES[p.region];
                let street = pick(rng, EN_STREETS);
                let suffix = pick(rng, EN_STREET_SUFFIXES);
                text(format!(
                    "{} {street} {suffix}, {city}, {state} {zip}{:02}",
                    p.street_no,
                    rng.random_range(0..100)
                ))
            }
        }
        Kind::Zip => text(if zh {
            format!(
                "{}{:02}",
                ZH_REGIONS[p.region].zip,
                rng.random_range(0..100)
            )
        } else {
            format!("{}{:02}", EN_CITIES[p.region].3, rng.random_range(0..100))
        }),
        Kind::Company => text(if zh {
            let city = ZH_REGIONS[p.region].city.trim_end_matches('市');
            format!(
                "{city}{}{}{}",
                pick(rng, ZH_COMPANY_PREFIXES),
                pick(rng, ZH_COMPANY_INDUSTRIES),
                pick(rng, ZH_COMPANY_SUFFIXES)
            )
        } else {
            format!(
                "{} {}",
                pick(rng, EN_COMPANY_WORDS),
                pick(rng, EN_COMPANY_SUFFIXES)
            )
        }),
        Kind::Job => text((*pick(rng, if zh { ZH_JOBS } else { EN_JOBS })).to_owned()),
        Kind::Url => text(format!(
            "https://{}.{}/{}",
            pick(rng, URL_WORDS),
            pick(rng, TLDS),
            p.handle.replace('.', "-")
        )),
        Kind::Ip => text(format!(
            "{}.{}.{}.{}",
            rng.random_range(1..224u8),
            rng.random::<u8>(),
            rng.random::<u8>(),
            rng.random_range(1..255u8)
        )),
        Kind::Ipv6 => {
            let groups: Vec<String> = (0..8)
                .map(|_| format!("{:x}", rng.random::<u16>()))
                .collect();
            text(groups.join(":"))
        }
        Kind::Mac => {
            let bytes: Vec<String> = (0..6)
                .map(|_| format!("{:02x}", rng.random::<u8>()))
                .collect();
            text(bytes.join(":"))
        }
        Kind::Color => text(format!("#{:06x}", rng.random_range(0..0x100_0000u32))),
        Kind::Password => {
            const CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789!@#$%";
            let length = rng.random_range(12..=16);
            text((0..length).map(|_| *pick(rng, CHARS) as char).collect())
        }
        Kind::Word => text((*pick(rng, if zh { ZH_WORDS } else { EN_WORDS })).to_owned()),
        Kind::Sentence => text(sentence(rng, locale)),
        Kind::Paragraph => {
            let count = rng.random_range(3..=5);
            let parts: Vec<String> = (0..count).map(|_| sentence(rng, locale)).collect();
            text(parts.join(if zh { "" } else { " " }))
        }
        Kind::Constant => {
            let raw = field.options.clone().unwrap_or_default();
            // 数字常量按数字输出
            raw.trim()
                .parse::<i64>()
                .map(Value::from)
                .unwrap_or(Value::String(raw))
        }
    }
}

#[derive(Debug)]
pub struct Generated {
    pub seed: u64,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

/// 生成前 limit 行（limit 为 None 时生成全部）
pub fn generate(schema: &Schema, limit: Option<usize>, today: Date) -> PluginResult<Generated> {
    validate(schema)?;
    let seed = schema.seed.unwrap_or_else(rand::random);
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let age = schema
        .fields
        .iter()
        .find(|f| f.kind == Kind::Age)
        .map(|f| {
            (
                f.min.unwrap_or(18.0).max(0.0) as u32,
                f.max.unwrap_or(60.0).max(0.0) as u32,
            )
        })
        .unwrap_or((18, 60));
    let count = limit.map_or(schema.rows, |l| l.min(schema.rows));
    let mut rows = Vec::with_capacity(count);
    for row in 0..count {
        let p = person(&mut rng, schema.locale, age, today);
        let values = schema
            .fields
            .iter()
            .map(|field| {
                // 先生成再决定是否为空，保证空值比例变化时其他数据不变
                let generated = value(field, row, &p, schema.locale, today, &mut rng);
                let null = field.null_percent > 0
                    && rng.random_range(0..100u8) < field.null_percent.min(100);
                if null { Value::Null } else { generated }
            })
            .collect();
        rows.push(values);
    }
    Ok(Generated {
        seed,
        columns: schema
            .fields
            .iter()
            .map(|f| f.name.trim().to_owned())
            .collect(),
        rows,
    })
}

/// 把一行转为对象（保持字段顺序）
pub fn object(columns: &[String], row: &[Value]) -> Map<String, Value> {
    columns.iter().cloned().zip(row.iter().cloned()).collect()
}

#[cfg(test)]
#[path = "generate_test.rs"]
mod tests;
