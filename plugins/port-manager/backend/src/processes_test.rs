use std::process::Command;

use super::*;

#[test]
fn looks_up_own_process() {
    let me = std::process::id();
    let found = lookup(&[me, 999_999_999]);
    let info = &found[&me];
    assert!(
        info.name.contains("tfp_port_manager") || !info.name.is_empty(),
        "{info:?}"
    );
    assert!(info.memory > 0);
    assert!(!found.contains_key(&999_999_999));
    assert_eq!(clip("x".repeat(500)).chars().count(), MAX_COMMAND + 1);
}

#[test]
fn refuses_protected_processes() {
    for pid in [0, 1, std::process::id()] {
        let err = terminate(TerminateArgs { pid, force: true }).unwrap_err();
        assert_eq!(err.code, "port.protected_process");
    }
    let err = terminate(TerminateArgs {
        pid: 999_999_999,
        force: false,
    })
    .unwrap_err();
    assert_eq!(err.code, "port.process_not_found");
}

/// 启动一个会持续约 30 秒的子进程（Windows 没有 sleep 命令）
fn long_running_child() -> std::process::Child {
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("ping");
        command.args(["-n", "30", "127.0.0.1"]);
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut command = Command::new("sleep");
        command.arg("30");
        command
    };
    command.stdout(std::process::Stdio::null()).spawn().unwrap()
}

/// Unix 发送 SIGTERM；Windows 不支持时退回为结束进程
#[test]
fn terminates_a_child_process() {
    let mut child = long_running_child();
    terminate(TerminateArgs {
        pid: child.id(),
        force: false,
    })
    .unwrap();
    let status = child.wait().unwrap();
    assert!(!status.success());
}
