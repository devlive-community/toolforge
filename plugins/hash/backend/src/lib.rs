//! 哈希计算插件后端：文本摘要与 HMAC 实时计算，文件摘要以耗时任务运行并输出实时日志。

mod algo;
mod files;
mod hmac;

use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, parse_args, to_value,
    unknown_function,
};

pub use algo::Algorithm;
use algo::MultiHasher;

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct Hash {
    manifest: Manifest,
}

impl Default for Hash {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

#[derive(Deserialize)]
struct TextArgs {
    input: String,
    algorithms: Vec<Algorithm>,
    #[serde(default)]
    uppercase: bool,
}

#[derive(Serialize)]
struct Digest {
    algorithm: Algorithm,
    digest: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TextReport {
    results: Vec<Digest>,
    bytes: usize,
    elapsed_ms: f64,
}

fn hash_text(args: TextArgs) -> PluginResult<TextReport> {
    if args.algorithms.is_empty() {
        return Err(PluginError::new("hash.no_algorithms"));
    }
    let start = Instant::now();
    let mut hasher = MultiHasher::new(&args.algorithms);
    hasher.update(args.input.as_bytes());
    let results = hasher
        .finish(args.uppercase)
        .into_iter()
        .map(|(algorithm, digest)| Digest { algorithm, digest })
        .collect();
    Ok(TextReport {
        results,
        bytes: args.input.len(),
        elapsed_ms: (start.elapsed().as_secs_f64() * 100_000.0).round() / 100.0,
    })
}

impl ToolPlugin for Hash {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "hash_text" => to_value(hash_text(parse_args(args)?)?),
            "hmac_text" => to_value(hmac::hmac_text(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "hash_files" => to_value(files::run(parse_args(args)?, ctx)?),
            _ => self.call(function, args),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
