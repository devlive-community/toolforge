use super::*;

#[test]
fn lists_only_plugins_with_a_cli_interface() {
    let registry = crate::plugins::builtin();
    let plugins = cli_plugins(&registry, None);
    assert!(plugins.contains(&"hash".to_owned()));
    assert!(
        !plugins.contains(&"uuid".to_owned()),
        "plugins without declarations get no command"
    );
}

#[test]
fn skips_plugins_turned_off_for_the_command_line() {
    let registry = crate::plugins::builtin();
    let store = Store::open_in_memory().unwrap();
    store
        .interface_grant_set(&tf_core::InterfaceGrant {
            channel: "cli".into(),
            plugin_id: "org.devlive.toolforge.hash".into(),
            function: tf_core::store::PLUGIN_SCOPE.into(),
            mode: "off".into(),
            uses: None,
        })
        .unwrap();
    assert!(!cli_plugins(&registry, Some(&store)).contains(&"hash".to_owned()));
}
