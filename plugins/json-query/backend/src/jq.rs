//! 用 jaq 执行 jq 程序。jaq 无法中途打断，所以放在单独的线程里并设超时；输出个数与大小有上限。

use std::sync::mpsc;
use std::time::Duration;

use jaq_core::load::{Arena, File, Loader};
use jaq_core::{Compiler, Ctx, Vars, data, unwrap_valr};
use jaq_json::write::{Pp, write};
use std::rc::Rc;

use jaq_json::{Num, Val, read};
use tf_plugin_api::{PluginError, PluginResult};

use crate::{Options, Output, Results};

pub const TIMEOUT: Duration = Duration::from_secs(10);

/// 错误位置（1 开始的行列）：slice 指向 code 中的某处
fn position(code: &str, at: &str) -> (usize, usize) {
    let offset = (at.as_ptr() as usize)
        .checked_sub(code.as_ptr() as usize)
        .filter(|o| *o <= code.len())
        .unwrap_or(code.len());
    let before = &code[..offset];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    (line, column)
}

fn syntax(code: &str, expected: &str, at: &str) -> PluginError {
    let (line, column) = position(code, at);
    let found: String = at.chars().take(12).collect();
    PluginError::new("query.syntax")
        .with("expected", expected)
        .with("found", found)
        .with("line", line)
        .with("column", column)
}

/// 整数值的浮点数按整数输出（与 jq 一致：add / length 得到 31 而不是 31.0）
fn normalize(value: &Val) -> Val {
    match value {
        Val::Num(Num::Float(f))
            if f.is_finite() && f.fract() == 0.0 && f.abs() < 9_007_199_254_740_992.0 =>
        {
            Val::from(*f as isize)
        }
        Val::Arr(items) => Val::Arr(Rc::new(items.iter().map(normalize).collect())),
        Val::Obj(map) => Val::Obj(Rc::new(
            map.iter().map(|(k, v)| (k.clone(), normalize(v))).collect(),
        )),
        other => other.clone(),
    }
}

fn format(value: &Val, options: &Options) -> String {
    let value = &normalize(value);
    if options.raw
        && let Val::TStr(bytes) = value
    {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    let pp = Pp {
        indent: (!options.compact).then(|| "  ".to_owned()),
        sort_keys: options.sort_keys,
        // 与 jq 一致：缩进输出时冒号后加空格
        sep_space: !options.compact,
        ..Pp::default()
    };
    let mut out = Vec::new();
    let _ = write(&mut out, &pp, 0, value);
    String::from_utf8_lossy(&out).into_owned()
}

fn execute(code: &str, input: &str, options: &Options) -> PluginResult<Results> {
    let defs = jaq_core::defs()
        .chain(jaq_std::defs())
        .chain(jaq_json::defs());
    let funs = jaq_core::funs()
        .chain(jaq_std::funs())
        .chain(jaq_json::funs());
    let loader = Loader::new(defs);
    let arena = Arena::default();
    let modules = loader
        .load(&arena, File { code, path: () })
        .map_err(|errors| {
            use jaq_core::load::Error;
            match errors.into_iter().next().map(|(_, e)| e) {
                Some(Error::Lex(errs)) => errs
                    .first()
                    .map(|(expect, at)| syntax(code, expect.as_str(), at))
                    .unwrap_or_else(|| PluginError::new("query.syntax")),
                Some(Error::Parse(errs)) => errs
                    .first()
                    .map(|(expect, at)| syntax(code, expect.as_str(), at))
                    .unwrap_or_else(|| PluginError::new("query.syntax")),
                _ => PluginError::new("query.syntax"),
            }
        })?;
    let filter = Compiler::default()
        .with_funs(funs)
        .compile(modules)
        .map_err(|errors| {
            let first = errors
                .into_iter()
                .next()
                .and_then(|(_, errs)| errs.into_iter().next());
            match first {
                Some((name, undefined)) => {
                    let (line, column) = position(code, name);
                    // 按类型给出不同的错误码，便于翻译
                    let code = match undefined.as_str() {
                        "filter" => "query.undefined_filter",
                        "variable" => "query.undefined_variable",
                        _ => "query.undefined",
                    };
                    PluginError::new(code)
                        .with("name", name)
                        .with("line", line)
                        .with("column", column)
                }
                None => PluginError::new("query.syntax"),
            }
        })?;

    let mut results = Results::default();
    for value in read::parse_many(input.as_bytes()) {
        let value = value
            .map_err(|e| PluginError::new("query.invalid_json").with("detail", e.to_string()))?;
        let ctx = Ctx::<data::JustLut<Val>>::new(&filter.lut, Vars::new([]));
        for out in filter.id.run((ctx, value)).map(unwrap_valr) {
            let out =
                out.map_err(|e| PluginError::new("query.runtime").with("detail", e.to_string()))?;
            if !results.push(Output {
                path: None,
                text: format(&out, options),
            }) {
                return Ok(results);
            }
        }
    }
    Ok(results)
}

/// 在新线程中执行，超时则放弃等待
pub fn run(code: &str, input: String, options: Options) -> PluginResult<Results> {
    if code.trim().is_empty() {
        return Err(PluginError::new("query.empty_query"));
    }
    let (tx, rx) = mpsc::channel();
    let code = code.to_owned();
    std::thread::Builder::new()
        .name("jq".into())
        .spawn(move || {
            let _ = tx.send(execute(&code, &input, &options));
        })
        .map_err(|e| PluginError::new("query.runtime").with("detail", e.to_string()))?;
    rx.recv_timeout(TIMEOUT).unwrap_or_else(|_| {
        Err(PluginError::new("query.timeout").with("seconds", TIMEOUT.as_secs()))
    })
}

#[cfg(test)]
#[path = "jq_test.rs"]
mod tests;
