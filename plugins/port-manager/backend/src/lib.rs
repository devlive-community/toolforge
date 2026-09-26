//! 端口管理插件后端：列出本机监听的端口及所属进程、结束进程，以及检查远程端口是否可达。

mod check;
mod detect;
mod processes;
mod sockets;

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginResult, TaskContext, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

#[derive(Deserialize)]
struct ListArgs {
    #[serde(default)]
    connections: bool,
    /// 端口、进程名、PID、地址或命令行中包含的文字
    #[serde(default)]
    query: String,
    #[serde(default)]
    protocol: Option<sockets::Protocol>,
}

#[derive(Serialize)]
struct Listing {
    sockets: Vec<sockets::Socket>,
    processes: Vec<processes::ProcessInfo>,
    /// 过滤前的套接字数量
    total: usize,
}

fn matches(
    socket: &sockets::Socket,
    query: &str,
    processes: &std::collections::HashMap<u32, processes::ProcessInfo>,
) -> bool {
    if query.is_empty() {
        return true;
    }
    socket.port.to_string().starts_with(query)
        || socket.local_addr.contains(query)
        || socket
            .remote_addr
            .as_deref()
            .is_some_and(|a| a.contains(query))
        || socket.pids.iter().any(|pid| {
            pid.to_string() == query
                || processes.get(pid).is_some_and(|p| {
                    p.name.to_lowercase().contains(query)
                        || p.command
                            .as_deref()
                            .is_some_and(|c| c.to_lowercase().contains(query))
                })
        })
}

fn list(args: ListArgs) -> PluginResult<Listing> {
    let all = sockets::list(args.connections)?;
    let total = all.len();
    let pids: Vec<u32> = all
        .iter()
        .flat_map(|s| s.pids.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let found = processes::lookup(&pids);
    let query = args.query.trim().to_lowercase();
    let sockets: Vec<_> = all
        .into_iter()
        .filter(|s| args.protocol.is_none_or(|p| s.protocol == p))
        .filter(|s| matches(s, &query, &found))
        .collect();
    let shown: BTreeSet<u32> = sockets
        .iter()
        .flat_map(|s| s.pids.iter().copied())
        .collect();
    let mut processes: Vec<_> = found
        .into_values()
        .filter(|p| shown.contains(&p.pid))
        .collect();
    processes.sort_by_key(|p| p.pid);
    Ok(Listing {
        sockets,
        processes,
        total,
    })
}

pub struct PortManager {
    manifest: Manifest,
}

impl Default for PortManager {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for PortManager {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn detect(&self, text: &str) -> Option<tf_plugin_api::Detection> {
        detect::detect(text)
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "list" => to_value(list(parse_args(args)?)?),
            "terminate" => {
                processes::terminate(parse_args(args)?)?;
                Ok(Value::Null)
            }
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "check" => to_value(check::run(parse_args(args)?, ctx)?),
            _ => self.call(function, args),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
