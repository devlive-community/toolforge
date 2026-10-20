//! 系统代理配置查看器：只读采集多来源配置事实。
//! 区分「配置了什么」与「某个程序实际怎么连」：不下载 PAC、不做 WPAD、
//! 不探测连通性、不修改设置。Rust 负责解析与脱敏，凭据不落历史。

mod env;
mod mask;
mod model;
mod platform;

use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct ProxyInspector {
    manifest: Manifest,
}

impl Default for ProxyInspector {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

/// 采集全部来源与环境变量；只在打开/手动刷新时调用
fn inspect() -> model::Report {
    let sources = platform::collect_sources()
        .into_iter()
        .map(|result| match result {
            Ok(source) => source,
            // 读取失败：保留来源占位与错误码，绝不显示成直连
            Err(failure) => model::Source {
                id: failure.id,
                scope: "system".to_owned(),
                status: model::SourceStatus::Failed { code: failure.code },
                entries: vec![],
                auto_discover: None,
                pac_url: None,
                exclusions: vec![],
                raw: None,
            },
        })
        .collect();
    model::Report {
        sources,
        env_vars: env::collect(),
        env_available: true,
    }
}

impl ToolPlugin for ProxyInspector {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, _args: Value) -> PluginResult<Value> {
        match function {
            "inspect" => to_value(inspect()),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
