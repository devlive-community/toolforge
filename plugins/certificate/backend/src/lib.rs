//! 证书查看插件后端：解析 PEM / DER 证书与证书链，获取服务器的 TLS 证书链并用系统信任库校验。

mod parse;
mod tls;

use std::io::Read;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Detection, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, parse_args, to_value,
    unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");
const MAX_INPUT: usize = 1024 * 1024;

#[derive(Deserialize)]
struct TextArgs {
    text: String,
}

#[derive(Deserialize)]
struct FileArgs {
    path: String,
}

#[derive(Serialize)]
struct Parsed {
    certificates: Vec<parse::Certificate>,
    ordered: bool,
}

fn parse_bytes(bytes: &[u8]) -> PluginResult<Parsed> {
    if bytes.len() > MAX_INPUT {
        return Err(PluginError::new("cert.too_large").with("limit", "1 MB"));
    }
    let now = jiff::Timestamp::now().as_second();
    let certificates = parse::split(bytes)?
        .iter()
        .map(|der| parse::certificate(der, now))
        .collect::<PluginResult<Vec<_>>>()?;
    Ok(Parsed {
        ordered: parse::chain_ordered(&certificates),
        certificates,
    })
}

fn read_file(path: &str) -> PluginResult<Vec<u8>> {
    let file = std::fs::File::open(path).map_err(|e| {
        let code = if e.kind() == std::io::ErrorKind::NotFound {
            "fs.not_found"
        } else {
            "fs.io"
        };
        PluginError::new(code).with("path", path)
    })?;
    let mut bytes = Vec::new();
    file.take(MAX_INPUT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| PluginError::new("fs.io").with("detail", e.to_string()))?;
    Ok(bytes)
}

pub struct CertificateViewer {
    manifest: Manifest,
}

impl Default for CertificateViewer {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for CertificateViewer {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn detect(&self, text: &str) -> Option<Detection> {
        let count = text.matches("-----BEGIN CERTIFICATE-----").count();
        (count > 0).then(|| Detection::new(98, "pem").with("count", count))
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "parse" => {
                let args: TextArgs = parse_args(args)?;
                to_value(parse_bytes(args.text.trim().as_bytes())?)
            }
            "parse_file" => {
                let args: FileArgs = parse_args(args)?;
                to_value(parse_bytes(&read_file(&args.path)?)?)
            }
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "fetch" => to_value(tls::fetch(parse_args(args)?, ctx)?),
            _ => self.call(function, args),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
