//! 数据模型：各配置来源的快照、状态与脱敏。Rust 负责解析与脱敏，界面只展示。

use serde::{Deserialize, Serialize};

/// 单个来源快照
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    /// 来源标识：macos-sc | win-inet | win-http | gnome
    pub id: String,
    /// 作用范围：user | system（前端翻译）
    pub scope: String,
    pub status: SourceStatus,
    /// 启用的协议条目；禁用/未配置/失败时为空
    pub entries: Vec<Entry>,
    /// 自动发现开关（WPAD）；来源不区分时为 None
    pub auto_discover: Option<bool>,
    /// PAC 地址；已遮罩凭据
    pub pac_url: Option<String>,
    /// 排除列表
    pub exclusions: Vec<String>,
    /// 原始配置（已脱敏，供展开核对）；读取失败时为 None
    pub raw: Option<serde_json::Value>,
}

/// 来源读取状态：区分配置了什么、禁用、未配置、失败与不支持
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SourceStatus {
    /// 至少一个协议启用
    Configured,
    /// 留有地址但全局禁用
    Disabled,
    /// 没有任何配置
    NotConfigured,
    /// 读取失败：错误码由前端翻译
    Failed { code: String },
    /// 当前平台不支持该来源
    Unsupported,
}

/// 单协议代理条目
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// http | https | socks | pac
    pub protocol: String,
    /// 已遮罩凭据
    pub host: String,
    pub port: u32,
    /// 格式提示：端口越界、地址空白等（错误码）
    pub warnings: Vec<String>,
}

/// 来源读取结果：错误不吞成“直连”，失败来源保留占位
pub type SourceResult = Result<Source, SourceFailure>;

/// 读取失败：来源标识 + 错误码（前端翻译）
#[derive(Debug)]
pub struct SourceFailure {
    pub id: String,
    pub code: String,
}

/// 完整报告
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub sources: Vec<Source>,
    /// 进程启动继承的代理环境变量（大小写并存，各值保留）
    pub env_vars: Vec<EnvVar>,
    /// 环境变量来源不可用时为 false（理论上只有异常情况）
    pub env_available: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvVar {
    pub name: String,
    /// 已遮罩凭据
    pub value: String,
    /// NO_PROXY 等排除列表按逗号拆分展示
    pub is_exclusions: bool,
}

/// 排除列表条目上限；仅 Windows 与 Linux 来源使用
#[cfg_attr(not(any(windows, target_os = "linux")), expect(dead_code))]
pub const MAX_EXCLUSIONS: usize = 200;

/// 组装来源：按条目推断状态（配置/禁用/未配置）
pub fn build_source(
    id: &str,
    scope: &str,
    entries: Vec<Entry>,
    auto_discover: Option<bool>,
    pac_url: Option<String>,
    exclusions: Vec<String>,
    raw: Option<serde_json::Value>,
) -> Source {
    let status = if entries.is_empty() && pac_url.is_none() && auto_discover != Some(true) {
        // 留有地址但开关关闭属于 Disabled，由调用方用 raw 判断不了，因此这里只覆盖“空”
        if has_residue(&raw) {
            SourceStatus::Disabled
        } else {
            SourceStatus::NotConfigured
        }
    } else {
        SourceStatus::Configured
    };
    Source {
        id: id.to_owned(),
        scope: scope.to_owned(),
        status,
        entries,
        auto_discover,
        pac_url,
        exclusions,
        raw,
    }
}

/// 禁用但留有地址：raw 里存在任意非空标量/数组
fn has_residue(raw: &Option<serde_json::Value>) -> bool {
    match raw {
        Some(serde_json::Value::Object(map)) => map.values().any(|value| match value {
            serde_json::Value::Null => false,
            serde_json::Value::Array(items) => !items.is_empty(),
            serde_json::Value::String(text) => !text.trim().is_empty(),
            serde_json::Value::Bool(_) | serde_json::Value::Number(_) => false,
            _ => true,
        }),
        _ => false,
    }
}

/// 端口与地址校验：只给格式提示，不判定可达
pub fn check_entry(host: &str, port: u32) -> Vec<String> {
    let mut warnings = Vec::new();
    if host.trim().is_empty() {
        warnings.push("proxy.empty_host".to_owned());
    }
    if port == 0 || port > 65535 {
        warnings.push("proxy.bad_port".to_owned());
    }
    warnings
}

#[cfg(test)]
#[path = "model_test.rs"]
mod tests;
