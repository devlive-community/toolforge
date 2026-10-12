use super::*;

#[test]
fn exposes_the_convert_task() {
    let plugin = SvgConverter::default();
    assert_eq!(plugin.manifest().id, "org.devlive.toolforge.svg-converter");
    assert!(plugin.call("convert", serde_json::json!({})).is_err());
}
