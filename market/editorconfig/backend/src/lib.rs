//! EditorConfig 生效规则检查器（在线安装插件）：解释项目中某个文件最终使用了哪些 .editorconfig 规则、
//! 来自哪个文件哪一行、被谁覆盖。只读，不改写配置也不格式化源码；只读取用户选择的项目目录内的配置。

mod config;
mod glob;
mod report;
mod resolve;

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");
/// 一次检查的目标文件上限
const MAX_TARGETS: usize = 200;

pub struct EditorConfigTool {
    manifest: Manifest,
}

impl Default for EditorConfigTool {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

#[derive(Deserialize)]
struct ChainArgs {
    root: String,
    targets: Vec<String>,
}

#[derive(Serialize)]
struct ConfigFile {
    /// 相对项目根的目录，根目录为空
    dir: String,
    /// 要读取的 .editorconfig 绝对路径（沿用项目根的分隔符风格）
    path: String,
}

#[derive(Deserialize)]
struct RelativeArgs {
    root: String,
    paths: Vec<String>,
}

fn separator(root: &str) -> char {
    if root.contains('\\') { '\\' } else { '/' }
}

/// 需要读取的 .editorconfig：所有目标的上级目录，止于项目根
fn chain(args: ChainArgs) -> PluginResult<Vec<ConfigFile>> {
    if args.targets.len() > MAX_TARGETS {
        return Err(PluginError::new("ec.too_many").with("max", MAX_TARGETS));
    }
    let mut dirs = BTreeSet::new();
    for target in &args.targets {
        dirs.extend(resolve::ancestors(&resolve::normalize(target)?));
    }
    let sep = separator(&args.root);
    let root = args.root.trim().trim_end_matches(['/', '\\']);
    Ok(dirs
        .into_iter()
        .map(|dir| {
            let mut path = root.to_owned();
            for segment in dir.split('/').filter(|s| !s.is_empty()) {
                path.push(sep);
                path.push_str(segment);
            }
            path.push(sep);
            path.push_str(".editorconfig");
            ConfigFile { dir, path }
        })
        .collect())
}

/// 把选中的绝对路径转为相对项目根的路径；不在项目根内的拒绝
fn relative(args: RelativeArgs) -> PluginResult<Vec<String>> {
    let unify = |p: &str| p.trim().replace('\\', "/");
    let root = unify(&args.root);
    let root = root.trim_end_matches('/');
    // Windows 盘符与路径不区分大小写
    let windows = root.get(1..2) == Some(":");
    args.paths
        .iter()
        .map(|path| {
            let unified = unify(path);
            let head = unified.get(..root.len()).unwrap_or_default();
            let same = if windows {
                head.eq_ignore_ascii_case(root)
            } else {
                head == root
            };
            let rest = unified.get(root.len()..).unwrap_or_default();
            if !same || !rest.starts_with('/') {
                return Err(PluginError::new("ec.outside_root").with("path", path.as_str()));
            }
            resolve::normalize(&rest[1..])
        })
        .collect()
}

impl ToolPlugin for EditorConfigTool {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "chain" => to_value(chain(parse_args(args)?)?),
            "relative" => to_value(relative(parse_args(args)?)?),
            "resolve" => {
                let args: resolve::Args = parse_args(args)?;
                if args.targets.len() > MAX_TARGETS {
                    return Err(PluginError::new("ec.too_many").with("max", MAX_TARGETS));
                }
                to_value(resolve::run(args)?)
            }
            "report" => to_value(report::run(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(EditorConfigTool);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
