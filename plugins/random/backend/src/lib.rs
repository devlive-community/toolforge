//! 随机工具插件后端：随机整数、名单抽签（可加权、不放回）、打乱、分组、骰子与硬币。
//! 可填写公开的种子，同样的种子与输入总是得到同样的结果，便于他人验证抽签公平。

mod ops;
mod rng;

use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

fn one() -> usize {
    1
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NumbersArgs {
    min: i64,
    max: i64,
    #[serde(default = "one")]
    count: usize,
    #[serde(default)]
    unique: bool,
    #[serde(default)]
    sorted: bool,
    #[serde(default)]
    seed: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListArgs {
    items: String,
    #[serde(default = "one")]
    count: usize,
    #[serde(default)]
    weighted: bool,
    /// 分组：组数或每组人数
    #[serde(default)]
    groups: Option<usize>,
    #[serde(default)]
    size: Option<usize>,
    #[serde(default = "yes")]
    shuffle: bool,
    #[serde(default)]
    seed: Option<String>,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiceArgs {
    #[serde(default = "one")]
    count: usize,
    #[serde(default = "six")]
    sides: u32,
    #[serde(default)]
    seed: Option<String>,
}

fn six() -> u32 {
    6
}

fn seeded(seed: &Option<String>) -> bool {
    seed.as_deref().is_some_and(|s| !s.trim().is_empty())
}

pub struct RandomTool {
    manifest: Manifest,
}

impl Default for RandomTool {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for RandomTool {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "numbers" => {
                let a: NumbersArgs = parse_args(args)?;
                let mut rng = rng::make("numbers", a.seed.as_deref());
                let values = ops::numbers(&mut rng, a.min, a.max, a.count, a.unique, a.sorted)?;
                let sum: i128 = values.iter().map(|v| *v as i128).sum();
                Ok(
                    json!({ "values": values, "sum": sum.to_string(), "seeded": seeded(&a.seed), "algorithm": rng::VERSION }),
                )
            }
            "draw" | "shuffle" | "groups" => {
                let a: ListArgs = parse_args(args)?;
                let items = ops::parse_items(&a.items, a.weighted && function == "draw")?;
                let mut rng = rng::make(function, a.seed.as_deref());
                let result = match function {
                    "draw" => json!({ "winners": ops::draw(&mut rng, &items, a.count)? }),
                    "shuffle" => json!({ "items": ops::shuffle(&mut rng, &items) }),
                    _ => {
                        json!({ "groups": ops::groups(&mut rng, &items, a.groups, a.size, a.shuffle)? })
                    }
                };
                let mut result = result;
                result["total"] = json!(items.len());
                result["seeded"] = json!(seeded(&a.seed));
                result["algorithm"] = json!(rng::VERSION);
                Ok(result)
            }
            "dice" => {
                let a: DiceArgs = parse_args(args)?;
                let mut rng = rng::make("dice", a.seed.as_deref());
                let rolls = ops::dice(&mut rng, a.count, a.sides)?;
                let sum: u64 = rolls.iter().map(|r| *r as u64).sum();
                Ok(
                    json!({ "rolls": rolls, "sum": sum, "seeded": seeded(&a.seed), "algorithm": rng::VERSION }),
                )
            }
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
