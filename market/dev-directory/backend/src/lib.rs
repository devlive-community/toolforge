//! 开发者导航插件后端：离线精选站点清单，按分类与关键词检索。
//! 自定义条目由前端经 plugin_state 保存，不进后端数据。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct DevDirectory {
    manifest: Manifest,
}

impl Default for DevDirectory {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for DevDirectory {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "list" => to_value(list(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(DevDirectory);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Site {
    pub name: &'static str,
    pub url: &'static str,
    /// categories.<key>
    pub category: &'static str,
    /// descriptions.<slug>
    pub description: &'static str,
}

macro_rules! sites {
    ($(($cat:ident, $name:literal, $url:literal, $desc:literal)),+ $(,)?) => {
        &[ $( Site { name: $name, url: $url, category: stringify!($cat), description: $desc } ),+ ]
    };
}

static SITES: &[Site] = sites![
    (docs, "MDN Web Docs", "https://developer.mozilla.org", "mdn"),
    (docs, "Go", "https://go.dev/doc/", "go"),
    (docs, "Rust", "https://doc.rust-lang.org/book/", "rust"),
    (docs, "Python", "https://docs.python.org/3/", "python"),
    (
        docs,
        "TypeScript",
        "https://www.typescriptlang.org/docs/",
        "typescript"
    ),
    (docs, "React", "https://react.dev", "react"),
    (docs, "Vue", "https://vuejs.org/guide/", "vue"),
    (docs, "Tauri", "https://v2.tauri.app", "tauri"),
    (
        docs,
        "PostgreSQL",
        "https://www.postgresql.org/docs/",
        "postgres"
    ),
    (docs, "Docker", "https://docs.docker.com", "docker"),
    (docs, "Kubernetes", "https://kubernetes.io/docs/", "k8s"),
    (tools, "Regex101", "https://regex101.com", "regex101"),
    (tools, "Can I Use", "https://caniuse.com", "caniuse"),
    (tools, "JWT Debugger", "https://jwt.io", "jwt"),
    (tools, "Crontab Guru", "https://crontab.guru", "cron"),
    (
        tools,
        "URL Encode",
        "https://www.urlencoder.org",
        "urlencode"
    ),
    (
        tools,
        "Explain Shell",
        "https://explainshell.com",
        "explainshell"
    ),
    (
        tools,
        "HTTP Status Dogs",
        "https://httpstatusdogs.com",
        "httpstatus"
    ),
    (
        tools,
        "Nanoid",
        "https://zelark.github.io/nano-id-cc/",
        "nanoid"
    ),
    (
        tools,
        "TLS Checker",
        "https://www.ssllabs.com/ssltest/",
        "ssltest"
    ),
    (assets, "Google Fonts", "https://fonts.google.com", "fonts"),
    (
        assets,
        "Iconify",
        "https://icon-sets.iconify.design",
        "iconify"
    ),
    (assets, "Lucide Icons", "https://lucide.dev", "lucide"),
    (assets, "Coolors", "https://coolors.co", "coolors"),
    (assets, "Heroicons", "https://heroicons.com", "heroicons"),
    (assets, "Unsplash", "https://unsplash.com", "unsplash"),
    (
        assets,
        "Feather Icons",
        "https://feathericons.com",
        "feather"
    ),
    (
        learn,
        "freeCodeCamp",
        "https://www.freecodecamp.org",
        "freecodecamp"
    ),
    (
        learn,
        "The Odin Project",
        "https://www.theodinproject.com",
        "odin"
    ),
    (
        learn,
        "Rustlings",
        "https://github.com/rust-lang/rustlings",
        "rustlings"
    ),
    (
        learn,
        "Frontend Masters",
        "https://frontendmasters.com",
        "frontendmasters"
    ),
    (learn, "roadmap.sh", "https://roadmap.sh", "roadmap"),
    (learn, "LeetCode", "https://leetcode.com", "leetcode"),
    (learn, "Advent of Code", "https://adventofcode.com", "aoc"),
    (
        learn,
        "MDN Curriculum",
        "https://curriculum.mdn.plus",
        "curriculum"
    ),
    (platform, "GitHub", "https://github.com", "github"),
    (
        platform,
        "Stack Overflow",
        "https://stackoverflow.com",
        "stackoverflow"
    ),
    (platform, "crates.io", "https://crates.io", "crates"),
    (platform, "npm", "https://www.npmjs.com", "npm"),
    (platform, "PyPI", "https://pypi.org", "pypi"),
    (
        platform,
        "Docker Hub",
        "https://hub.docker.com",
        "dockerhub"
    ),
    (
        platform,
        "Hacker News",
        "https://news.ycombinator.com",
        "hn"
    ),
    (platform, "dev.to", "https://dev.to", "devto"),
];

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListArgs {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub category: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListOutput {
    pub sites: Vec<Site>,
    pub total: usize,
}

pub fn list(args: ListArgs) -> PluginResult<ListOutput> {
    let query = args.query.trim().to_lowercase();
    let sites: Vec<Site> = SITES
        .iter()
        .filter(|s| args.category.as_ref().is_none_or(|c| s.category == c))
        .filter(|s| {
            query.is_empty()
                || s.name.to_lowercase().contains(&query)
                || s.url.contains(&query)
                || s.description.contains(&query)
        })
        .cloned()
        .collect();
    if sites.is_empty() && !query.is_empty() {
        return Err(PluginError::new("directory.not_found").with("query", args.query.trim()));
    }
    let total = sites.len();
    Ok(ListOutput { sites, total })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
