//! 命令行的插件开关：插件与函数默认开启，用户在工具市场关闭后 tfp 不再提供。

use tf_core::store::PLUGIN_SCOPE;
use tf_core::{AppResult, InterfaceGrant, Store};

/// 授权表中的通道名
pub const CHANNEL: &str = "cli";

#[derive(Debug, Default)]
pub struct CliAccess {
    off: Vec<InterfaceGrant>,
}

impl CliAccess {
    /// 没有数据库时一律开启（与没有任何设置相同）
    pub fn load(store: Option<&Store>) -> AppResult<Self> {
        let Some(store) = store else {
            return Ok(Self::default());
        };
        let off = store
            .interface_grants(CHANNEL)?
            .into_iter()
            .filter(|g| g.mode == "off")
            .collect();
        Ok(Self { off })
    }

    fn is_off(&self, plugin_id: &str, function: &str) -> bool {
        self.off
            .iter()
            .any(|g| g.plugin_id == plugin_id && g.function == function)
    }

    pub fn plugin_enabled(&self, plugin_id: &str) -> bool {
        !self.is_off(plugin_id, PLUGIN_SCOPE)
    }

    pub fn function_enabled(&self, plugin_id: &str, function: &str) -> bool {
        self.plugin_enabled(plugin_id) && !self.is_off(plugin_id, function)
    }
}

#[cfg(test)]
#[path = "cli_access_test.rs"]
mod tests;
