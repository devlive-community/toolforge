//! 内置插件的注册入口。
//!
//! 内置插件与后续远程安装的插件遵循同一套 `ToolPlugin` 契约；
//! 这里只负责把随应用分发的插件放进注册表，宿主其余部分不感知具体工具。

use std::sync::Arc;

use tf_core::PluginRegistry;

pub fn builtin() -> PluginRegistry {
    let mut registry = PluginRegistry::new();
    registry.register(Arc::new(tfp_json_formatter::JsonFormatter::default()));
    registry.register(Arc::new(tfp_hash::Hash::default()));
    registry.register(Arc::new(tfp_uuid::UuidTool::default()));
    registry.register(Arc::new(tfp_timestamp::Timestamp::default()));
    registry.register(Arc::new(tfp_regex::Regex::default()));
    registry.register(Arc::new(tfp_xml_formatter::XmlFormatter::default()));
    registry
}
