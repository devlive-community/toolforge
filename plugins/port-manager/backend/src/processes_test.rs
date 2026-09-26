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

#[cfg(unix)]
#[test]
fn terminates_a_child_process() {
    let mut child = Command::new("sleep").arg("30").spawn().unwrap();
    terminate(TerminateArgs {
        pid: child.id(),
        force: false,
    })
    .unwrap();
    let status = child.wait().unwrap();
    assert!(!status.success());
}
