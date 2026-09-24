//! 系统监控插件后端：操作系统、CPU、内存、磁盘、网络、温度与进程信息，由 Rust 定时采样。

mod monitor;

use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct SystemMonitor {
    manifest: Manifest,
    monitor: monitor::Monitor,
}

impl Default for SystemMonitor {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
            monitor: monitor::Monitor::default(),
        }
    }
}

impl ToolPlugin for SystemMonitor {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "snapshot" => to_value(self.monitor.snapshot(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
