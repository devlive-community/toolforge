//! 内置插件的注册入口。
//!
//! 内置插件与后续远程安装的插件遵循同一套 `ToolPlugin` 契约；
//! 这里只负责把随应用分发的插件放进注册表，宿主其余部分不感知具体工具。

use tf_core::PluginRegistry;

pub fn builtin() -> PluginRegistry {
    PluginRegistry::new()
}
