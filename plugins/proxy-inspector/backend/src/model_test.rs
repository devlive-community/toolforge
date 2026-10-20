use crate::model;

#[test]
fn validates_ports_and_hosts() {
    assert_eq!(model::check_entry("", 8080), vec!["proxy.empty_host"]);
    assert_eq!(model::check_entry("proxy.corp", 0), vec!["proxy.bad_port"]);
    assert_eq!(
        model::check_entry("proxy.corp", 70_000),
        vec!["proxy.bad_port"]
    );
    assert!(model::check_entry("proxy.corp", 8080).is_empty());
}

#[test]
fn infers_status_from_entries_and_residue() {
    // 有条目：已配置
    let source = model::build_source(
        "x",
        "user",
        vec![model::Entry {
            protocol: "http".to_owned(),
            host: "p".into(),
            port: 8080,
            warnings: vec![],
        }],
        None,
        None,
        vec![],
        None,
    );
    assert_eq!(source.status, model::SourceStatus::Configured);
    // 无条目 + 空原始数据：未配置
    let source = model::build_source(
        "x",
        "user",
        vec![],
        None,
        None,
        vec![],
        Some(serde_json::json!({})),
    );
    assert_eq!(source.status, model::SourceStatus::NotConfigured);
    // 无条目但留有地址：禁用
    let source = model::build_source(
        "x",
        "user",
        vec![],
        Some(false),
        None,
        vec![],
        Some(serde_json::json!({"proxy": "proxy.corp:8080"})),
    );
    assert_eq!(source.status, model::SourceStatus::Disabled);
    // 仅 PAC 启用也算已配置
    let source = model::build_source(
        "x",
        "user",
        vec![],
        None,
        Some("http://host/pac".into()),
        vec![],
        None,
    );
    assert_eq!(source.status, model::SourceStatus::Configured);
}
