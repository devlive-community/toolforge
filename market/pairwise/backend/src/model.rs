use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tf_plugin_api::{PluginError, PluginResult};

pub const MAX_INPUT: usize = 8 * 1024 * 1024;
pub const MAX_COMBINATIONS: usize = 50_000;
pub const MAX_CASES: usize = 1_000;
pub const ALGORITHM: &str = "exhaustive-greedy-v1";

pub fn error(code: &str) -> PluginError {
    PluginError::new(format!("pairwise.{code}"))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Factor {
    pub name: String,
    pub values: Vec<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Term {
    pub factor: String,
    pub value: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Model {
    pub factors: Vec<Factor>,
    #[serde(default)]
    pub forbidden: Vec<Vec<Term>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftFactor {
    pub name: String,
    pub values_text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Draft {
    pub factors: Vec<DraftFactor>,
    pub forbidden_text: String,
}

pub struct Compiled {
    pub model: Model,
    pub constraints: Vec<Vec<(usize, usize)>>,
    pub combinations: usize,
}

/// Canonical decimal coefficient/exponent, without passing through f64.
/// This also makes 1, 1.0 and 1e0 duplicates while keeping "1" distinct.
fn number_text(text: &str) -> PluginResult<String> {
    if text.len() > 80 {
        return Err(error("scalar"));
    }
    let (negative, text) = text.strip_prefix('-').map_or((false, text), |t| (true, t));
    let (mantissa, exponent) = match text.split_once(['e', 'E']) {
        Some((m, e)) => (m, e.parse::<i32>().map_err(|_| error("scalar"))?),
        None => (text, 0),
    };
    if exponent.unsigned_abs() > 1_000 {
        return Err(error("scalar"));
    }
    let fraction = mantissa.split_once('.').map_or(0, |(_, f)| f.len()) as i32;
    let digits = mantissa.replace('.', "");
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Ok("0".to_owned());
    }
    let coefficient = digits.trim_end_matches('0');
    let power = exponent - fraction + (digits.len() - coefficient.len()) as i32;
    if power.unsigned_abs() > 1_000 {
        return Err(error("scalar"));
    }
    let sign = if negative { "-" } else { "" };
    let scientific = if power == 0 {
        format!("{sign}{coefficient}")
    } else {
        format!("{sign}{coefficient}e{power}")
    };
    // Choose a stable shortest spelling. Expanding "...0" to "...e1"
    // must not push an otherwise valid 80-character number over the limit.
    let position = coefficient.len() as i32 + power;
    let plain = if power >= 0 {
        format!("{sign}{coefficient}{}", "0".repeat(power as usize))
    } else if position > 0 {
        let (head, tail) = coefficient.split_at(position as usize);
        format!("{sign}{head}.{tail}")
    } else {
        format!("{sign}0.{}{coefficient}", "0".repeat((-position) as usize))
    };
    let canonical = if plain.len() <= scientific.len() {
        plain
    } else {
        scientific
    };
    if canonical.len() > 80 {
        return Err(error("scalar"));
    }
    Ok(canonical)
}

pub fn scalar(value: &Value) -> PluginResult<(String, Value)> {
    match value {
        Value::Null => Ok(("0:null".into(), Value::Null)),
        Value::Bool(flag) => Ok((format!("1:{flag}"), value.clone())),
        Value::Number(number) => {
            let text = number_text(&number.to_string())?;
            Ok((
                format!("2:{text}"),
                serde_json::from_str(&text).map_err(|_| error("scalar"))?,
            ))
        }
        Value::String(text) if text.len() <= 240 => Ok((format!("3:{text}"), value.clone())),
        _ => Err(error("scalar")),
    }
}

pub fn parse_json<T: serde::de::DeserializeOwned>(text: &str) -> PluginResult<T> {
    if text.len() > MAX_INPUT {
        return Err(error("input_limit"));
    }
    serde_json::from_str(text.strip_prefix('\u{feff}').unwrap_or(text)).map_err(|e| {
        error("json")
            .with("line", e.line())
            .with("column", e.column())
    })
}

pub fn from_draft(draft: Draft) -> PluginResult<Model> {
    let mut factors = Vec::new();
    if draft.factors.len() > 12 {
        return Err(error("factors"));
    }
    for factor in draft.factors {
        let values = parse_json(&factor.values_text)?;
        factors.push(Factor {
            name: factor.name,
            values,
        });
    }
    Ok(Model {
        factors,
        forbidden: parse_json(&draft.forbidden_text)?,
    })
}

pub fn normalize(mut model: Model) -> PluginResult<Compiled> {
    if !(2..=12).contains(&model.factors.len()) {
        return Err(error("factors"));
    }
    let mut names = BTreeSet::new();
    for factor in &mut model.factors {
        factor.name = factor.name.trim().to_owned();
        if factor.name.is_empty()
            || factor.name.len() > 80
            || factor.name.chars().any(char::is_control)
        {
            return Err(error("factor_name"));
        }
        if !names.insert(factor.name.clone()) {
            return Err(error("duplicate_factor").with("name", factor.name.clone()));
        }
        if !(1..=12).contains(&factor.values.len()) {
            return Err(error("values").with("name", factor.name.clone()));
        }
        let mut values = BTreeMap::new();
        for value in &factor.values {
            let (key, value) = scalar(value)?;
            if values.insert(key, value).is_some() {
                return Err(error("duplicate_value").with("name", factor.name.clone()));
            }
        }
        factor.values = values.into_values().collect();
    }
    model.factors.sort_by(|a, b| a.name.cmp(&b.name));
    let combinations = model
        .factors
        .iter()
        .try_fold(1usize, |n, f| n.checked_mul(f.values.len()))
        .ok_or_else(|| error("combination_limit"))?;
    if combinations > MAX_COMBINATIONS {
        return Err(error("combination_limit")
            .with("limit", MAX_COMBINATIONS)
            .with("combinations", combinations));
    }
    if model.forbidden.len() > 100 {
        return Err(error("constraints"));
    }
    let mut constraints = BTreeSet::new();
    for tuple in &model.forbidden {
        if tuple.is_empty() || tuple.len() > model.factors.len() {
            return Err(error("constraint_reference"));
        }
        let mut terms = BTreeMap::new();
        for term in tuple {
            let factor = model
                .factors
                .iter()
                .position(|f| f.name == term.factor.trim())
                .ok_or_else(|| error("constraint_reference"))?;
            let canonical = scalar(&term.value)?.1;
            let value = model.factors[factor]
                .values
                .iter()
                .position(|v| v == &canonical)
                .ok_or_else(|| error("constraint_reference"))?;
            if terms.insert(factor, value).is_some() {
                return Err(error("constraint_reference"));
            }
        }
        constraints.insert(terms.into_iter().collect::<Vec<_>>());
    }
    let constraints = constraints.into_iter().collect::<Vec<_>>();
    model.forbidden = constraints
        .iter()
        .map(|tuple| {
            tuple
                .iter()
                .map(|&(f, v)| Term {
                    factor: model.factors[f].name.clone(),
                    value: model.factors[f].values[v].clone(),
                })
                .collect()
        })
        .collect();
    Ok(Compiled {
        model,
        constraints,
        combinations,
    })
}

impl Compiled {
    pub fn allowed(&self, row: &[usize]) -> bool {
        !self
            .constraints
            .iter()
            .any(|tuple| tuple.iter().all(|&(f, v)| row[f] == v))
    }

    pub fn model_text(&self) -> String {
        serde_json::to_string_pretty(&self.model).expect("scalar model serializes")
    }

    pub fn draft(&self) -> Value {
        json!({
            "factors": self.model.factors.iter().map(|f| DraftFactor {
                name: f.name.clone(), values_text: serde_json::to_string(&f.values).expect("scalars"),
            }).collect::<Vec<_>>(),
            "forbiddenText": serde_json::to_string_pretty(&self.model.forbidden).expect("constraints"),
            "modelText": self.model_text(),
            "combinations": self.combinations,
        })
    }
}

#[cfg(test)]
#[path = "model_test.rs"]
mod tests;
