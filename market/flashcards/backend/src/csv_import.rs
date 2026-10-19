//! UTF-8 CSV 导入：固定列 question,answer,deck,tags（tags 用分号分隔）。
//! 支持 RFC 4180 引号与双引号转义；首行为表头（question 开头）时跳过。
//! 单行错误只收集不中断，最后统一返回给前端翻译。

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::schedule::{MAX_CSV_ROWS, MAX_TAGS, MAX_TEXT};

fn errp(code: &str, key: &str, value: impl Into<serde_json::Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportArgs {
    pub csv: String,
    /// 未提供 deck 列时使用的默认牌组
    #[serde(default)]
    pub default_deck: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImportedCard {
    pub question: String,
    pub answer: String,
    pub deck: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RowError {
    pub line: i64,
    pub rule: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportOut {
    pub cards: Vec<ImportedCard>,
    pub errors: Vec<RowError>,
}

/// 解析一行 CSV（RFC 4180）：返回字段列表；非法引号返回 None
fn parse_row(line: &str) -> Option<Vec<String>> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    let mut in_quotes = false;
    while let Some(ch) = chars.next() {
        if in_quotes {
            match ch {
                '"' => {
                    if chars.peek() == Some(&'"') {
                        chars.next();
                        current.push('"');
                    } else {
                        in_quotes = false;
                    }
                }
                _ => current.push(ch),
            }
        } else {
            match ch {
                ',' => {
                    fields.push(std::mem::take(&mut current));
                }
                '"' if current.is_empty() => in_quotes = true,
                '"' => return None,
                _ => current.push(ch),
            }
        }
    }
    if in_quotes {
        return None;
    }
    fields.push(current);
    Some(fields)
}

/// 导入 CSV：空问题/超长/超限行报错但继续；总数超上限时停止并报错
pub fn import_csv(args: ImportArgs) -> PluginResult<ImportOut> {
    let default_deck = if args.default_deck.trim().is_empty() {
        "default".to_owned()
    } else {
        args.default_deck.trim().to_owned()
    };
    let mut out = ImportOut {
        cards: Vec::new(),
        errors: Vec::new(),
    };
    for (index, line) in args.csv.lines().enumerate() {
        let line_number = index as i64 + 1;
        if line.trim().is_empty() {
            continue;
        }
        if out.cards.len() >= MAX_CSV_ROWS {
            return Err(errp("fcs.too_many_rows", "max", MAX_CSV_ROWS));
        }
        let Some(fields) = parse_row(line) else {
            out.errors.push(RowError {
                line: line_number,
                rule: "fcs.bad_quote".to_owned(),
            });
            continue;
        };
        // 表头：第一列为 question 且第二列为 answer
        if index == 0
            && fields.len() >= 2
            && fields[0].trim().eq_ignore_ascii_case("question")
            && fields[1].trim().eq_ignore_ascii_case("answer")
        {
            continue;
        }
        let question = fields
            .first()
            .map(|s| s.trim().to_owned())
            .unwrap_or_default();
        let answer = fields
            .get(1)
            .map(|s| s.trim().to_owned())
            .unwrap_or_default();
        if question.is_empty() {
            out.errors.push(RowError {
                line: line_number,
                rule: "fcs.empty_question".to_owned(),
            });
            continue;
        }
        if question.chars().count() > MAX_TEXT || answer.chars().count() > MAX_TEXT {
            out.errors.push(RowError {
                line: line_number,
                rule: "fcs.text_too_long".to_owned(),
            });
            continue;
        }
        let deck = fields
            .get(2)
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| default_deck.clone());
        let tags: Vec<String> = fields
            .get(3)
            .map(|s| {
                s.split(';')
                    .map(|tag| tag.trim().to_owned())
                    .filter(|tag| !tag.is_empty())
                    .take(MAX_TAGS)
                    .collect()
            })
            .unwrap_or_default();
        out.cards.push(ImportedCard {
            question,
            answer,
            deck,
            tags,
        });
    }
    Ok(out)
}

#[cfg(test)]
#[path = "csv_test.rs"]
mod tests;
