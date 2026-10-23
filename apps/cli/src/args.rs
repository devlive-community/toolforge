//! 命令行参数 → 插件入参：完全按接口声明里的参数 Schema 解析。
//!
//! - 选项：`--name value`、`--name=value`，同时接受 kebab-case（`--key-encoding`）
//! - 布尔：`--flag`、`--flag=false`、`--no-flag`；数组：逗号分隔或重复同一选项
//! - 位置参数按 `x-positional` 顺序；数组类型的位置参数吞掉剩余的位置参数
//! - `x-stdin` 参数没给出时读取标准输入（不是终端时）
//! - `--args '<JSON>'` / `--args @文件` 作为基础入参，选项覆盖其中同名字段

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use tf_core::{AppError, AppResult};

/// 所有插件函数共用的选项
#[derive(Debug, Default, PartialEq)]
pub struct Global {
    pub help: bool,
    pub json: bool,
    pub verbose: bool,
    pub lang: Option<String>,
    pub args: Option<String>,
}

/// 从参数中取出通用选项，返回其余参数；`--` 之后的内容原样保留
pub fn split_global(argv: &[String]) -> AppResult<(Global, Vec<String>)> {
    let mut global = Global::default();
    let mut rest = Vec::new();
    let mut iter = argv.iter();
    while let Some(arg) = iter.next() {
        let (name, inline) = match arg.split_once('=') {
            Some((name, value)) if name.starts_with("--") => (name, Some(value.to_owned())),
            _ => (arg.as_str(), None),
        };
        let mut value = |option: &str| {
            inline
                .clone()
                .or_else(|| iter.next().cloned())
                .ok_or_else(|| AppError::new("cli.missing_value").with("option", option))
        };
        match name {
            "--" => {
                rest.push(arg.clone());
                rest.extend(iter.cloned());
                break;
            }
            "-h" | "--help" => global.help = true,
            "--json" => global.json = true,
            "-v" | "--verbose" => global.verbose = true,
            "--lang" => global.lang = Some(value("--lang")?),
            "--args" => global.args = Some(value("--args")?),
            _ => rest.push(arg.clone()),
        }
    }
    Ok((global, rest))
}

/// 声明为路径的参数，已转换为绝对路径；宿主据此做路径授权
#[derive(Debug, Clone, PartialEq)]
pub struct PathUse {
    pub param: String,
    pub path: PathBuf,
    pub write: bool,
}

#[derive(Debug, PartialEq)]
pub struct Parsed {
    pub args: Value,
    pub paths: Vec<PathUse>,
}

/// camelCase → kebab-case
pub fn kebab(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            out.push('-');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn properties(params: &Value) -> Vec<(&String, &Value)> {
    params["properties"]
        .as_object()
        .map(|p| p.iter().collect())
        .unwrap_or_default()
}

/// 位置参数，按 x-positional 排序
pub fn positionals(params: &Value) -> Vec<(&String, &Value)> {
    let mut out: Vec<(&String, &Value)> = properties(params)
        .into_iter()
        .filter(|(_, schema)| schema.get("x-positional").is_some())
        .collect();
    out.sort_by_key(|(_, schema)| schema["x-positional"].as_u64());
    out
}

fn invalid(option: &str, value: &str) -> AppError {
    AppError::new("cli.invalid_value")
        .with("option", option)
        .with("value", value)
}

/// 把一个字符串按标量类型转换
fn scalar(kind: &str, option: &str, raw: &str) -> AppResult<Value> {
    match kind {
        "integer" => raw
            .parse::<i64>()
            .map(Value::from)
            .map_err(|_| invalid(option, raw)),
        "number" => raw
            .parse::<f64>()
            .ok()
            .and_then(|n| serde_json::Number::from_f64(n).map(Value::Number))
            .ok_or_else(|| invalid(option, raw)),
        "boolean" => match raw.to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Ok(Value::Bool(true)),
            "false" | "0" | "no" => Ok(Value::Bool(false)),
            _ => Err(invalid(option, raw)),
        },
        _ => Ok(Value::String(raw.to_owned())),
    }
}

/// 赋值：数组追加（支持逗号分隔），其余覆盖
fn assign(
    args: &mut Map<String, Value>,
    name: &str,
    schema: &Value,
    option: &str,
    raw: &str,
    appended: &mut Vec<String>,
) -> AppResult<()> {
    if schema["type"] == "array" {
        let item = schema["items"]["type"].as_str().unwrap_or("string");
        // --args 里给出的数组被命令行选项替换，而不是在其后追加
        if !appended.iter().any(|n| n == name) {
            args.insert(name.to_owned(), Value::Array(Vec::new()));
            appended.push(name.to_owned());
        }
        let list = args
            .get_mut(name)
            .and_then(Value::as_array_mut)
            .expect("array inserted above");
        for part in raw.split(',').filter(|p| !p.is_empty()) {
            list.push(scalar(item, option, part)?);
        }
        return Ok(());
    }
    let kind = schema["type"].as_str().unwrap_or("string");
    args.insert(name.to_owned(), scalar(kind, option, raw)?);
    Ok(())
}

fn base_args(raw: Option<&str>, cwd: &Path) -> AppResult<Map<String, Value>> {
    let Some(raw) = raw else {
        return Ok(Map::new());
    };
    let text = match raw.strip_prefix('@') {
        Some(file) => std::fs::read_to_string(cwd.join(file)).map_err(|e| {
            AppError::new("cli.args_file")
                .with("path", file)
                .with("detail", e.to_string())
        })?,
        None => raw.to_owned(),
    };
    match serde_json::from_str(&text) {
        Ok(Value::Object(map)) => Ok(map),
        _ => Err(AppError::new("cli.args_json")),
    }
}

/// 解析一个函数的参数。stdin 只在需要时才调用
pub fn parse(
    params: &Value,
    argv: &[String],
    global_args: Option<&str>,
    cwd: &Path,
    stdin: impl FnOnce() -> Option<String>,
) -> AppResult<Parsed> {
    let props = properties(params);
    let mut args = base_args(global_args, cwd)?;
    let mut appended = Vec::new();
    let mut loose = Vec::new();
    let mut only_positional = false;
    let mut iter = argv.iter();

    while let Some(arg) = iter.next() {
        if only_positional || !arg.starts_with("--") {
            loose.push(arg.clone());
            continue;
        }
        if arg == "--" {
            only_positional = true;
            continue;
        }
        let (flag, inline) = match arg.split_once('=') {
            Some((flag, value)) => (flag, Some(value)),
            None => (arg.as_str(), None),
        };
        let wanted = &flag[2..];
        let find = |wanted: &str| {
            props
                .iter()
                .find(|(name, _)| *name == wanted || kebab(name) == wanted)
                .copied()
        };
        if let Some(negated) = wanted.strip_prefix("no-")
            && inline.is_none()
            && let Some((name, schema)) = find(negated)
            && schema["type"] == "boolean"
        {
            args.insert(name.clone(), Value::Bool(false));
            continue;
        }
        let Some((name, schema)) = find(wanted) else {
            return Err(AppError::new("cli.unknown_option").with("option", flag));
        };
        if schema["type"] == "boolean" && inline.is_none() {
            args.insert(name.clone(), Value::Bool(true));
            continue;
        }
        let raw = match inline {
            Some(value) => value.to_owned(),
            None => iter
                .next()
                .cloned()
                .ok_or_else(|| AppError::new("cli.missing_value").with("option", flag))?,
        };
        assign(&mut args, name, schema, flag, &raw, &mut appended)?;
    }

    let mut loose = loose.into_iter().peekable();
    for (name, schema) in positionals(params) {
        if loose.peek().is_none() {
            break;
        }
        if schema["type"] == "array" {
            for raw in loose.by_ref() {
                assign(&mut args, name, schema, name, &raw, &mut appended)?;
            }
        } else if let Some(raw) = loose.next() {
            assign(&mut args, name, schema, name, &raw, &mut appended)?;
        }
    }
    if let Some(extra) = loose.next() {
        return Err(AppError::new("cli.unexpected_argument").with("value", extra));
    }

    if let Some((name, _)) = props
        .iter()
        .find(|(name, schema)| schema["x-stdin"] == true && !args.contains_key(*name))
        && let Some(text) = stdin()
    {
        args.insert((*name).clone(), Value::String(text));
    }

    for (name, schema) in &props {
        if !args.contains_key(*name)
            && let Some(default) = schema.get("default")
        {
            args.insert((*name).clone(), default.clone());
        }
    }
    for required in params["required"].as_array().into_iter().flatten() {
        let name = required.as_str().unwrap_or_default();
        if !args.contains_key(name) {
            return Err(AppError::new("cli.missing_required").with("name", kebab(name)));
        }
    }

    let mut paths = Vec::new();
    for (name, schema) in &props {
        let Some(value) = args.get_mut(*name) else {
            continue;
        };
        let item = if schema["type"] == "array" {
            &schema["items"]
        } else {
            schema
        };
        if let Some(allowed) = item["enum"].as_array() {
            let values: Vec<&Value> = match &*value {
                Value::Array(list) => list.iter().collect(),
                other => vec![other],
            };
            if let Some(bad) = values.into_iter().find(|v| !allowed.contains(v)) {
                let shown = bad.as_str().map_or_else(|| bad.to_string(), str::to_owned);
                let options: Vec<String> = allowed
                    .iter()
                    .map(|a| a.as_str().map_or_else(|| a.to_string(), str::to_owned))
                    .collect();
                return Err(AppError::new("cli.invalid_choice")
                    .with("option", format!("--{}", kebab(name)))
                    .with("value", shown)
                    .with("allowed", options.join(", ")));
            }
        }
        if item["format"] == "path" {
            let write = item["x-access"] == "write";
            let mut absolutize = |v: &mut Value| {
                if let Some(raw) = v.as_str() {
                    let path = cwd.join(raw);
                    *v = Value::String(path.to_string_lossy().into_owned());
                    paths.push(PathUse {
                        param: (*name).clone(),
                        path,
                        write,
                    });
                }
            };
            match value {
                Value::Array(list) => list.iter_mut().for_each(&mut absolutize),
                other => absolutize(other),
            }
        }
    }

    Ok(Parsed {
        args: Value::Object(args),
        paths,
    })
}

#[cfg(test)]
#[path = "args_test.rs"]
mod tests;
