//! 调用历史：桌面端、命令行与 AI Agent 共用同一套记录规则，并标明来源。
//! sensitive 插件和未声明 history 的函数不记录；用户关闭历史后都不记录。

use serde_json::Value;
use tf_core::{Store, history, store::now_millis};
use tf_plugin_api::Manifest;

/// 历史开关在 kv 中的键，默认开启
pub const ENABLED_KEY: &str = "history.enabled";

pub fn enabled(store: &Store) -> bool {
    store
        .kv_get(ENABLED_KEY)
        .ok()
        .flatten()
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
}

pub fn records(manifest: &Manifest, function: &str) -> bool {
    !manifest.sensitive && manifest.functions.get(function).is_some_and(|f| f.history)
}

/// 记录一次成功的调用；失败只写日志，不影响调用结果
pub fn record(
    store: &Store,
    source: &str,
    manifest: &Manifest,
    function: &str,
    args: &Value,
    result: &Value,
) {
    if !records(manifest, function) || !enabled(store) {
        return;
    }
    let Some(input) = history::primary_text(args) else {
        return;
    };
    let output = history::output_preview(result);
    let outcome = store.history_record_from(
        source,
        &manifest.id,
        function,
        &input,
        output.as_deref(),
        now_millis(),
    );
    if let Err(err) = outcome {
        log::warn!("history record failed: {}", err.code);
    }
}

#[cfg(test)]
#[path = "history_test.rs"]
mod tests;
