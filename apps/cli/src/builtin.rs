//! 编进 tfp 的内置插件：只收声明了 CLI 接口的内置插件，其余内置插件不能从命令行调用。
//! 内置插件新增 CLI 接口时在这里登记（测试会检查遗漏），在线插件从安装目录加载，不需要登记。

use std::sync::Arc;

use tf_plugin_api::ToolPlugin;

pub struct Builtin {
    pub create: fn() -> Arc<dyn ToolPlugin>,
    /// 语言 → 语言包 JSON
    pub locales: &'static [(&'static str, &'static str)],
}

pub fn all() -> Vec<Builtin> {
    vec![Builtin {
        create: || Arc::new(tfp_hash::Hash::default()),
        locales: &[
            (
                "en-US",
                include_str!("../../../plugins/hash/locales/en-US.json"),
            ),
            (
                "zh-CN",
                include_str!("../../../plugins/hash/locales/zh-CN.json"),
            ),
        ],
    }]
}

#[cfg(test)]
#[path = "builtin_test.rs"]
mod tests;
