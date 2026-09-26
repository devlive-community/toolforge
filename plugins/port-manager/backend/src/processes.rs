//! 进程信息与结束进程。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, Signal, System, UpdateKind, Users};
use tf_plugin_api::{PluginError, PluginResult};

/// 命令行最多返回的字符数
const MAX_COMMAND: usize = 400;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub exe: Option<String>,
    pub command: Option<String>,
    pub user: Option<String>,
    pub memory: u64,
    /// Unix 秒
    pub start_time: u64,
}

fn clip(text: String) -> String {
    match text.char_indices().nth(MAX_COMMAND) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text,
    }
}

pub fn lookup(pids: &[u32]) -> HashMap<u32, ProcessInfo> {
    let mut system = System::new();
    let targets: Vec<Pid> = pids.iter().map(|&p| Pid::from_u32(p)).collect();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&targets),
        true,
        ProcessRefreshKind::nothing()
            .with_memory()
            .with_user(UpdateKind::OnlyIfNotSet)
            .with_exe(UpdateKind::OnlyIfNotSet)
            .with_cmd(UpdateKind::OnlyIfNotSet),
    );
    let users = Users::new_with_refreshed_list();
    targets
        .iter()
        .filter_map(|pid| system.process(*pid))
        .map(|p| {
            let command = p
                .cmd()
                .iter()
                .map(|part| part.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ");
            let info = ProcessInfo {
                pid: p.pid().as_u32(),
                name: p.name().to_string_lossy().into_owned(),
                exe: p.exe().map(|e| e.to_string_lossy().into_owned()),
                command: (!command.is_empty()).then(|| clip(command)),
                user: p
                    .user_id()
                    .and_then(|uid| users.get_user_by_id(uid))
                    .map(|u| u.name().to_owned()),
                memory: p.memory(),
                start_time: p.start_time(),
            };
            (info.pid, info)
        })
        .collect()
}

#[derive(Debug, Deserialize)]
pub struct TerminateArgs {
    pub pid: u32,
    /// 强制结束（SIGKILL / TerminateProcess）；否则请求进程正常退出
    #[serde(default)]
    pub force: bool,
}

/// 不允许结束的进程：系统进程与 ToolForge 自身
pub fn protected(pid: u32) -> bool {
    pid <= 1 || pid == std::process::id()
}

pub fn terminate(args: TerminateArgs) -> PluginResult<()> {
    if protected(args.pid) {
        return Err(PluginError::new("port.protected_process").with("pid", args.pid));
    }
    let mut system = System::new();
    let pid = Pid::from_u32(args.pid);
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing(),
    );
    let process = system
        .process(pid)
        .ok_or_else(|| PluginError::new("port.process_not_found").with("pid", args.pid))?;
    let sent = if args.force {
        process.kill()
    } else {
        // 不支持 SIGTERM 的平台（Windows）退回为结束进程
        process
            .kill_with(Signal::Term)
            .unwrap_or_else(|| process.kill())
    };
    if sent {
        Ok(())
    } else {
        Err(PluginError::new("port.terminate_failed").with("pid", args.pid))
    }
}

#[cfg(test)]
#[path = "processes_test.rs"]
mod tests;
