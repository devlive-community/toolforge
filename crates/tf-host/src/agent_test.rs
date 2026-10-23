use serde_json::json;

use super::*;

const PLUGIN: &str = "org.devlive.toolforge.demo";

fn interface(effect: &str, uses: &[&str], confirm: bool) -> Interface {
    serde_json::from_value(json!({
        "expose": ["agent"],
        "summary": "s",
        "effect": effect,
        "uses": uses,
        "confirm": confirm,
        "params": { "type": "object" }
    }))
    .unwrap()
}

fn grant(function: &str, mode: &str, uses: Option<&[&str]>) -> InterfaceGrant {
    InterfaceGrant {
        channel: CHANNEL.into(),
        plugin_id: PLUGIN.into(),
        function: function.into(),
        mode: mode.into(),
        uses: uses.map(|u| u.iter().map(|s| (*s).to_owned()).collect()),
    }
}

fn policy(grants: Vec<InterfaceGrant>, paths: &[AgentPath]) -> Policy {
    Policy::new(true, grants, paths)
}

#[test]
fn global_switch_and_plugin_switch_come_first() {
    let read = interface("read", &[], false);
    let off = Policy::new(false, vec![grant(PLUGIN_SCOPE, "allow", None)], &[]);
    assert_eq!(
        off.decide(PLUGIN, "f", &read, false, &[]),
        Decision::Deny(AppError::new("agent.disabled"))
    );
    let on_by_default = Policy::new(true, Vec::new(), &[]);
    assert!(
        on_by_default.plugin_enabled(PLUGIN),
        "plugins are enabled until the user turns them off"
    );
    let no_plugin = Policy::new(true, vec![grant(PLUGIN_SCOPE, "off", None)], &[]);
    assert!(
        matches!(no_plugin.decide(PLUGIN, "f", &read, false, &[]), Decision::Deny(e) if e.code == "agent.plugin_off")
    );
}

#[test]
fn defaults_follow_the_effect() {
    let policy = policy(Vec::new(), &[]);
    assert_eq!(
        policy.decide(PLUGIN, "f", &interface("read", &[], false), false, &[]),
        Decision::Allow
    );
    assert_eq!(
        policy.decide(PLUGIN, "f", &interface("write", &[], false), false, &[]),
        Decision::Ask
    );
    assert_eq!(
        policy.decide(PLUGIN, "f", &interface("external", &[], false), false, &[]),
        Decision::Ask
    );
}

#[test]
fn explicit_modes_are_capped_by_the_declaration() {
    let write = interface("write", &[], false);
    let allowed = policy(vec![grant("f", "allow", Some(&[]))], &[]);
    assert_eq!(
        allowed.decide(PLUGIN, "f", &write, false, &[]),
        Decision::Allow
    );

    let confirm = interface("write", &[], true);
    assert_eq!(
        allowed.decide(PLUGIN, "f", &confirm, false, &[]),
        Decision::Ask,
        "confirm always asks"
    );

    let off = policy(vec![grant("f", "off", None)], &[]);
    assert!(
        matches!(off.decide(PLUGIN, "f", &write, false, &[]), Decision::Deny(e) if e.code == "agent.function_off")
    );
}

#[test]
fn widened_permissions_need_a_new_grant() {
    let uses_fs = interface("read", &[FS_PERMISSION], false);
    let granted_without = policy(vec![grant("f", "allow", Some(&[]))], &[]);
    assert_eq!(
        granted_without.function_mode(PLUGIN, "f", &uses_fs, false),
        Mode::Ask
    );
    let granted_with = policy(vec![grant("f", "allow", Some(&[FS_PERMISSION]))], &[]);
    assert_eq!(
        granted_with.function_mode(PLUGIN, "f", &uses_fs, false),
        Mode::Allow
    );
}

#[test]
fn native_plugins_with_file_access_always_ask() {
    let uses_fs = interface("read", &[FS_PERMISSION], false);
    let policy = policy(vec![grant("f", "allow", Some(&[FS_PERMISSION]))], &[]);
    assert_eq!(policy.function_mode(PLUGIN, "f", &uses_fs, true), Mode::Ask);
    assert_eq!(
        policy.function_mode(PLUGIN, "f", &uses_fs, false),
        Mode::Allow
    );
}

#[test]
fn paths_must_stay_inside_granted_folders() {
    let root = std::env::temp_dir().join(format!("tf-agent-paths-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let readable = root.join("readable");
    let writable = root.join("writable");
    let outside = root.join("outside");
    for dir in [&readable, &writable, &outside] {
        std::fs::create_dir_all(dir).unwrap();
    }
    std::fs::write(readable.join("a.txt"), "a").unwrap();
    std::fs::write(outside.join("secret"), "s").unwrap();
    let paths = [
        AgentPath {
            path: readable.to_string_lossy().into(),
            access: "read".into(),
        },
        AgentPath {
            path: writable.to_string_lossy().into(),
            access: "write".into(),
        },
        AgentPath {
            path: root.join("missing").to_string_lossy().into(),
            access: "write".into(),
        },
    ];
    let policy = policy(Vec::new(), &paths);
    assert_eq!(
        policy.roots().len(),
        2,
        "folders that do not exist are ignored"
    );

    assert!(policy.path_allowed(&readable.join("a.txt"), false));
    assert!(
        !policy.path_allowed(&readable.join("a.txt"), true),
        "read-only folders cannot be written"
    );
    assert!(
        policy.path_allowed(&writable.join("new.txt"), true),
        "new files in writable folders"
    );
    assert!(!policy.path_allowed(&outside.join("secret"), false));
    assert!(
        !policy.path_allowed(&readable.join("../outside/secret"), false),
        ".. is resolved first"
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(outside.join("secret"), readable.join("link")).unwrap();
        assert!(
            !policy.path_allowed(&readable.join("link"), false),
            "symlinks are resolved first"
        );
    }

    let scope = policy.file_scope();
    assert!(scope.allows(&readable.join("a.txt"), false));
    assert!(!scope.allows(&outside.join("secret"), false));
    assert!(scope.allows(&writable.join("new.txt"), true));

    let read = interface("read", &[FS_PERMISSION], false);
    let denied = policy.decide(
        PLUGIN,
        "f",
        &read,
        false,
        &[(outside.join("secret"), false)],
    );
    assert!(matches!(denied, Decision::Deny(e) if e.code == "agent.path_denied"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn loads_from_the_store() {
    let store = Store::open_in_memory().unwrap();
    assert!(
        !Policy::load(&store).unwrap().enabled(),
        "agents are off by default"
    );
    store.kv_set(ENABLED_KEY, &json!(true)).unwrap();
    store
        .interface_grant_set(&grant(PLUGIN_SCOPE, "off", None))
        .unwrap();
    let policy = Policy::load(&store).unwrap();
    assert!(policy.enabled() && !policy.plugin_enabled(PLUGIN));
}

#[test]
fn sensitive_arguments_are_masked() {
    let params = json!({ "properties": { "key": { "type": "string", "x-sensitive": true }, "input": { "type": "string" } } });
    let masked = mask_sensitive(&params, &json!({ "key": "secret", "input": "hi" }));
    assert_eq!(masked, json!({ "key": "***", "input": "hi" }));
}
