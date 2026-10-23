use super::*;

#[test]
fn lists_only_plugins_with_a_cli_interface() {
    let registry = crate::plugins::builtin();
    let plugins = cli_plugins(&registry);
    assert!(plugins.contains(&"hash".to_owned()));
    assert!(
        !plugins.contains(&"uuid".to_owned()),
        "plugins without declarations get no command"
    );
}
