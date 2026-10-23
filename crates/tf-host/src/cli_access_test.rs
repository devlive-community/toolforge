use super::*;

fn off(function: &str) -> InterfaceGrant {
    InterfaceGrant {
        channel: CHANNEL.into(),
        plugin_id: "p".into(),
        function: function.into(),
        mode: "off".into(),
        uses: None,
    }
}

#[test]
fn everything_is_enabled_by_default() {
    let access = CliAccess::load(None).unwrap();
    assert!(access.plugin_enabled("p") && access.function_enabled("p", "f"));
    let store = Store::open_in_memory().unwrap();
    let access = CliAccess::load(Some(&store)).unwrap();
    assert!(access.function_enabled("p", "f"));
}

#[test]
fn users_can_turn_off_plugins_and_functions() {
    let store = Store::open_in_memory().unwrap();
    store.interface_grant_set(&off("f")).unwrap();
    let access = CliAccess::load(Some(&store)).unwrap();
    assert!(access.plugin_enabled("p"));
    assert!(!access.function_enabled("p", "f"));
    assert!(access.function_enabled("p", "g"));

    store.interface_grant_set(&off(PLUGIN_SCOPE)).unwrap();
    let access = CliAccess::load(Some(&store)).unwrap();
    assert!(!access.plugin_enabled("p"));
    assert!(
        !access.function_enabled("p", "g"),
        "a disabled plugin disables every function"
    );
}
