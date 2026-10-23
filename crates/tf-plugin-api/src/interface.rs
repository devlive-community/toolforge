//! 函数对 CLI / AI Agent 的接口声明，对应 manifest 的 `functions.<fn>.interface`。
//!
//! manifest 中以原始 JSON 保存，宿主按需解析：声明有误或含未知取值时只是该函数不对外暴露，
//! 不会让整个 manifest 解析失败，旧版本宿主遇到新字段、新取值时仍能正常加载插件。
//! 严格校验由 `cargo xtask rules` 在开发期完成。

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 调用通道
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Cli,
    Agent,
    /// 新版本宿主才认识的通道；忽略，不影响其余通道
    #[serde(other)]
    Other,
}

/// 函数的副作用；宿主据此决定调用前是否需要用户授权或确认。
/// 不认识的取值会让整个声明无效（不暴露），宁可少暴露也不按错误的副作用授权。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Effect {
    /// 只读计算，不写文件、不联网、不改变持久状态
    Read,
    /// 写文件或修改持久数据
    Write,
    /// 对外产生影响：发网络请求、写剪贴板、启动进程等
    External,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Interface {
    pub expose: Vec<Channel>,
    /// 形如 `i18n:api.<fn>.summary`
    pub summary: String,
    pub effect: Effect,
    /// 本函数用到的权限，是插件 permissions 的子集
    #[serde(default)]
    pub uses: Vec<String>,
    /// 每次调用都需要用户确认
    #[serde(default)]
    pub confirm: bool,
    /// 参数的 JSON Schema（受限子集，见 xtask 校验）
    pub params: Value,
    /// CLI 文本输出提示：`{"text": "/指针"}` 或 `{"table": "/指针", "columns": [...]}`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<String>,
}

/// 一个函数的接口声明状态
#[derive(Debug, Clone, PartialEq)]
pub enum Declaration {
    /// 没有声明：规则生效前的旧插件，不对外暴露
    Undeclared,
    /// 显式写了 `"interface": false`：评估后决定不暴露
    Hidden,
    Declared(Box<Interface>),
    /// 声明存在但宿主无法理解，不对外暴露
    Invalid,
}

impl Declaration {
    pub fn parse(raw: Option<&Value>) -> Self {
        match raw {
            None | Some(Value::Null) => Self::Undeclared,
            Some(Value::Bool(false)) => Self::Hidden,
            Some(value @ Value::Object(_)) => serde_json::from_value(value.clone())
                .map_or(Self::Invalid, |interface| {
                    Self::Declared(Box::new(interface))
                }),
            Some(_) => Self::Invalid,
        }
    }

    /// 已声明且对该通道开放时返回声明
    pub fn exposed(&self, channel: Channel) -> Option<&Interface> {
        match self {
            Self::Declared(interface) if interface.expose.contains(&channel) => Some(interface),
            _ => None,
        }
    }
}

/// 按参数 Schema 构造一份入参：有默认值用默认值，必填参数取枚举首项或示例值
pub fn sample_args(params: &Value) -> Value {
    let required: Vec<&str> = params["required"]
        .as_array()
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let mut args = serde_json::Map::new();
    for (name, schema) in params["properties"].as_object().into_iter().flatten() {
        let value = if let Some(default) = schema.get("default") {
            default.clone()
        } else if !required.contains(&name.as_str()) {
            continue;
        } else if let Some(first) = schema.pointer("/enum/0") {
            first.clone()
        } else {
            match schema["type"].as_str() {
                Some("integer" | "number") => schema.get("minimum").cloned().unwrap_or(1.into()),
                Some("boolean") => false.into(),
                Some("array") => Value::Array(vec![
                    schema
                        .pointer("/items/enum/0")
                        .cloned()
                        .unwrap_or_else(|| "1".into()),
                ]),
                _ => "1".into(),
            }
        };
        args.insert(name.clone(), value);
    }
    Value::Object(args)
}

/// 供插件测试使用：用 [`sample_args`] 调用插件所有已声明、非任务、无路径参数的函数，
/// 声明的 Schema 必须能被后端接受（不能返回 `plugin.invalid_args`）。返回实际调用的函数数
pub fn check_declared_interfaces(plugin: &dyn crate::ToolPlugin) -> Result<usize, String> {
    let manifest = plugin.manifest();
    let mut checked = 0;
    for (function, spec) in &manifest.functions {
        let declaration = spec.declaration();
        if declaration == Declaration::Invalid {
            return Err(format!("{} {function}: invalid interface", manifest.id));
        }
        let Some(interface) = declaration.exposed(Channel::Cli) else {
            continue;
        };
        if spec.task || interface.params.to_string().contains("\"format\":\"path\"") {
            continue;
        }
        let args = sample_args(&interface.params);
        if let Err(err) = plugin.call(function, args.clone())
            && err.code == "plugin.invalid_args"
        {
            return Err(format!(
                "{} {function} rejected {args}: {:?}",
                manifest.id, err.params
            ));
        }
        checked += 1;
    }
    Ok(checked)
}

#[cfg(test)]
#[path = "interface_test.rs"]
mod tests;
