//! 单条 User-Agent 解析：依次匹配爬虫、浏览器、引擎、系统与设备规则，结果带上命中的规则。

use std::sync::OnceLock;

use regex::{Captures, Regex};
use serde::Serialize;

use crate::rules::{self, Rule};

struct Compiled {
    rule: &'static Rule,
    re: Regex,
    unless: Option<Regex>,
}

struct Tables {
    bots: Vec<Compiled>,
    browsers: Vec<Compiled>,
    engines: Vec<Compiled>,
    systems: Vec<Compiled>,
    devices: Vec<Compiled>,
    model: Regex,
}

fn compile(table: &'static [Rule]) -> Vec<Compiled> {
    table
        .iter()
        .map(|rule| Compiled {
            rule,
            // 规则库是内置常量，单元测试保证全部可编译
            re: Regex::new(rule.pattern).expect("built-in rule"),
            unless: rule.unless.map(|p| Regex::new(p).expect("built-in rule")),
        })
        .collect()
}

fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| Tables {
        bots: compile(rules::BOTS),
        browsers: compile(rules::BROWSERS),
        engines: compile(rules::ENGINES),
        systems: compile(rules::OPERATING_SYSTEMS),
        devices: compile(rules::DEVICES),
        model: Regex::new(r"\(([^)]*)\)").expect("built-in rule"),
    })
}

/// 命中的规则：id、原始正则与 UA 中被匹配的片段
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub rule: &'static str,
    pub pattern: &'static str,
    pub matched: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    pub name: String,
    pub version: Option<String>,
    pub hit: Hit,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bot {
    pub name: String,
    pub version: Option<String>,
    /// search / social / ai / seo / monitor / tool / headless / generic
    pub category: &'static str,
    pub hit: Hit,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// bot / tv / console / wearable / tablet / mobile / desktop / unknown
    pub kind: &'static str,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub hit: Option<Hit>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Parsed {
    pub browser: Option<Part>,
    pub engine: Option<Part>,
    pub os: Option<Part>,
    pub device: Device,
    pub bot: Option<Bot>,
    /// 解读提示：frozenPlatform / reducedModel / macMayBeIpad / windows11
    pub notes: Vec<&'static str>,
}

fn find<'a>(table: &'a [Compiled], ua: &'a str) -> Option<(&'a Compiled, Captures<'a>)> {
    table.iter().find_map(|c| {
        let caps = c.re.captures(ua)?;
        if c.unless.as_ref().is_some_and(|u| u.is_match(ua)) {
            return None;
        }
        Some((c, caps))
    })
}

fn clean_version(raw: &str) -> Option<String> {
    let version = raw.replace('_', ".");
    let version = version.trim_matches('.');
    (!version.is_empty()).then(|| version.to_owned())
}

fn hit(c: &Compiled, caps: &Captures) -> Hit {
    Hit {
        rule: c.rule.id,
        pattern: c.rule.pattern,
        matched: caps.get(0).map_or("", |m| m.as_str()).to_owned(),
    }
}

/// 名称与版本：name 为空的兜底规则第 1 组是名称、第 2 组是版本；其余规则取第一个非空捕获组为版本
fn name_version(c: &Compiled, caps: &Captures) -> (String, Option<String>) {
    if c.rule.name.is_empty() {
        let name = caps.get(1).map_or("", |m| m.as_str()).trim().to_owned();
        return (name, caps.get(2).and_then(|m| clean_version(m.as_str())));
    }
    let version = match c.rule.fixed {
        Some(fixed) => Some(fixed.to_owned()),
        None => caps
            .iter()
            .skip(1)
            .flatten()
            .find_map(|m| clean_version(m.as_str())),
    };
    (c.rule.name.to_owned(), version)
}

fn part(table: &[Compiled], ua: &str) -> Option<Part> {
    let (c, caps) = find(table, ua)?;
    let (name, version) = name_version(c, &caps);
    Some(Part {
        name,
        version,
        hit: hit(c, &caps),
    })
}

/// 括号中第一个不像平台说明的片段视为型号（Android 设备）
fn android_model(ua: &str) -> Option<String> {
    let caps = tables().model.captures(ua)?;
    caps[1]
        .split(';')
        .map(str::trim)
        .find(|token| {
            let lower = token.to_ascii_lowercase();
            !token.is_empty()
                && !["linux", "u", "wv", "mobile", "tablet", "x11"].contains(&lower.as_str())
                && !lower.starts_with("android")
                && !lower.starts_with("harmonyos")
                && !lower.starts_with("openharmony")
                && !lower.starts_with("hmscore")
                && !lower.starts_with("arm")
                && !is_language(token)
        })
        .map(|token| {
            token
                .split(" Build/")
                .next()
                .unwrap_or(token)
                .trim()
                .to_owned()
        })
}

/// zh-CN、en_us、zh 这类语言标记
fn is_language(token: &str) -> bool {
    let mut parts = token.split(['-', '_']);
    let lang = parts.next().unwrap_or_default();
    let region = parts.next();
    lang.len() == 2
        && lang.chars().all(|c| c.is_ascii_lowercase())
        && region.is_none_or(|r| r.len() == 2 && r.chars().all(|c| c.is_ascii_alphabetic()))
        && parts.next().is_none()
}

fn vendor_of(model: &str) -> Option<String> {
    rules::VENDORS
        .iter()
        .find(|(prefix, _)| model.starts_with(prefix))
        .map(|(_, vendor)| (*vendor).to_owned())
}

fn device(ua: &str, os: Option<&Part>, bot: bool, notes: &mut Vec<&'static str>) -> Device {
    let found = find(&tables().devices, ua);
    let kind = if bot {
        "bot"
    } else {
        found.as_ref().map_or("unknown", |(c, _)| c.rule.tag)
    };
    let hit = found.as_ref().map(|(c, caps)| hit(c, caps));
    let (vendor, model) = if ua.contains("iPhone") {
        (Some("Apple".to_owned()), Some("iPhone".to_owned()))
    } else if ua.contains("iPad") {
        (Some("Apple".to_owned()), Some("iPad".to_owned()))
    } else if ua.contains("iPod") {
        (Some("Apple".to_owned()), Some("iPod".to_owned()))
    } else if ua.contains("Macintosh") {
        (Some("Apple".to_owned()), Some("Mac".to_owned()))
    } else if os.is_some_and(|os| os.name == "Android" || os.name == "HarmonyOS") {
        match android_model(ua) {
            // UA 精简后型号固定为 K
            Some(model) if model == "K" => {
                notes.push("reducedModel");
                (None, None)
            }
            Some(model) => {
                let vendor = vendor_of(&model).or_else(|| {
                    (os.is_some_and(|os| os.name == "HarmonyOS")).then(|| "Huawei".to_owned())
                });
                (vendor, Some(model))
            }
            None => (None, None),
        }
    } else {
        (None, None)
    };
    Device {
        kind,
        vendor,
        model,
        hit,
    }
}

pub fn parse(raw: &str) -> Parsed {
    let ua = raw.trim();
    let t = tables();
    let bot = find(&t.bots, ua).map(|(c, caps)| {
        let (name, version) = name_version(c, &caps);
        Bot {
            name,
            version,
            category: c.rule.tag,
            hit: hit(c, &caps),
        }
    });
    let browser = part(&t.browsers, ua);
    let engine = part(&t.engines, ua);
    let os = part(&t.systems, ua);
    let mut notes = Vec::new();
    let device = device(ua, os.as_ref(), bot.is_some(), &mut notes);

    if let Some(os) = &os {
        if os.hit.rule == "os.windows-10" {
            notes.push("windows11");
        }
        // Chromium 系浏览器冻结了系统版本：Windows 固定 10.0，macOS 固定 10_15_7
        let blink = engine.as_ref().is_some_and(|e| e.name == "Blink");
        let frozen_mac = os.name == "macOS" && os.version.as_deref() == Some("10.15.7");
        if blink
            && (os.hit.rule == "os.windows-10" || frozen_mac || notes.contains(&"reducedModel"))
        {
            notes.push("frozenPlatform");
        }
        if os.name == "macOS"
            && browser
                .as_ref()
                .is_some_and(|b| b.name == "Safari" || b.name == "iOS WebView")
        {
            notes.push("macMayBeIpad");
        }
    }

    Parsed {
        browser,
        engine,
        os,
        device,
        bot,
        notes,
    }
}

/// 看起来像浏览器或客户端的 UA（用于剪贴板识别）
pub fn looks_like_user_agent(text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() || text.contains('\n') || text.len() > 2048 {
        return false;
    }
    if text.starts_with("Mozilla/") && text.contains('(') {
        return true;
    }
    parse(text)
        .bot
        .is_some_and(|bot| bot.category != "generic" && text.contains('/'))
}

#[cfg(test)]
#[path = "parse_test.rs"]
mod tests;
