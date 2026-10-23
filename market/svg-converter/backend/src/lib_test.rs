use super::*;

#[test]
fn exposes_the_convert_task() {
    let plugin = SvgConverter::default();
    assert_eq!(plugin.manifest().id, "org.devlive.toolforge.svg-converter");
    assert!(plugin.call("convert", serde_json::json!({})).is_err());
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&SvgConverter::default()).unwrap();
    assert_eq!(checked, 0);
}
