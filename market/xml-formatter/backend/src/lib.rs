//! XML 排版与 XPath 查询；查询结果在插件实例中缓存、分页并完整导出。
mod detect;
mod xml;
mod xpath;

use serde::Deserialize;
use serde_json::Value;
use std::sync::Mutex;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, cancelled, parse_args, to_value,
    unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");
#[derive(Default)]
struct State {
    generation: u64,
    report: Option<xpath::Report>,
}
pub struct XmlFormatter {
    manifest: Manifest,
    state: Mutex<State>,
}
impl Default for XmlFormatter {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
            state: Mutex::default(),
        }
    }
}
#[derive(Deserialize)]
struct PageArgs {
    handle: u64,
    #[serde(default)]
    offset: usize,
    #[serde(default = "page_size")]
    limit: usize,
}
fn page_size() -> usize {
    xpath::PAGE_SIZE
}
#[derive(Deserialize)]
struct CopyArgs {
    handle: u64,
    index: Option<usize>,
    #[serde(default)]
    fragment: bool,
}
#[derive(Deserialize)]
struct ExportArgs {
    handle: u64,
    path: String,
}
impl XmlFormatter {
    fn with_report<T>(
        &self,
        handle: u64,
        f: impl FnOnce(&xpath::Report) -> PluginResult<T>,
    ) -> PluginResult<T> {
        let cache = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let report = cache
            .report
            .as_ref()
            .filter(|r| r.handle == handle)
            .ok_or_else(|| PluginError::new("xpath.expired"))?;
        f(report)
    }
}
impl ToolPlugin for XmlFormatter {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    fn detect(&self, text: &str) -> Option<tf_plugin_api::Detection> {
        detect::detect(text)
    }
    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "format" => to_value(xml::process(parse_args(args)?, xml::Mode::Pretty)?),
            "minify" => to_value(xml::process(parse_args(args)?, xml::Mode::Minify)?),
            "validate" => to_value(xml::process(parse_args(args)?, xml::Mode::Pretty)?),
            "page" => {
                let a: PageArgs = parse_args(args)?;
                self.with_report(a.handle, |r| to_value(r.page(a.offset, a.limit)?))
            }
            "copy" => {
                let a: CopyArgs = parse_args(args)?;
                self.with_report(a.handle, |r| to_value(r.copy(a.index, a.fragment)?))
            }
            other => Err(unknown_function(other)),
        }
    }
    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "query" => {
                let handle = {
                    let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
                    state.generation += 1;
                    state.report = None;
                    state.generation
                };
                let mut report = xpath::query(parse_args(args)?, ctx, xpath::Limits::default())?;
                report.handle = handle;
                let page = to_value(report.page(0, xpath::PAGE_SIZE)?)?;
                let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
                if state.generation != handle {
                    return Err(PluginError::new("xpath.expired"));
                }
                state.report = Some(report);
                Ok(page)
            }
            "export" => {
                let a: ExportArgs = parse_args(args)?;
                if ctx.is_cancelled() {
                    return Err(cancelled());
                }
                let bytes = self.with_report(a.handle, |r| {
                    serde_json::to_vec_pretty(r)
                        .map_err(|_| PluginError::new("xpath.export_failed"))
                })?;
                if ctx.is_cancelled() {
                    return Err(cancelled());
                }
                tf_plugin_guest::net::write_file(&a.path, &bytes)?;
                to_value(serde_json::json!({"bytes": bytes.len()}))
            }
            other => self.call(other, args),
        }
    }
}
tf_plugin_guest::export_plugin!(XmlFormatter);
#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
