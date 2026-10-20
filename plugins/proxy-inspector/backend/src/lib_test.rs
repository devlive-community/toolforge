use super::*;

/// SCDynamicStore 在测试进程会抛 ObjC 异常（无 bundle 上下文），系统来源
/// 读取无法单测；这里验证数据模型的序列化往返与环境变量收集。
#[test]
fn report_serializes_all_states() {
    let report = model::Report {
        sources: vec![
            model::build_source(
                "macos-sc",
                "system",
                vec![model::Entry {
                    protocol: "http".to_owned(),
                    host: "proxy.corp".to_owned(),
                    port: 8080,
                    warnings: vec![],
                }],
                Some(false),
                None,
                vec!["localhost".to_owned()],
                Some(serde_json::json!({ "HTTPEnable": 1 })),
            ),
            model::Source {
                id: "win-inet".to_owned(),
                scope: "user".to_owned(),
                status: model::SourceStatus::Failed {
                    code: "proxy.wininet_failed".to_owned(),
                },
                entries: vec![],
                auto_discover: None,
                pac_url: None,
                exclusions: vec![],
                raw: None,
            },
            model::Source {
                id: "gnome".to_owned(),
                scope: "system".to_owned(),
                status: model::SourceStatus::Unsupported,
                entries: vec![],
                auto_discover: None,
                pac_url: None,
                exclusions: vec![],
                raw: None,
            },
        ],
        env_vars: vec![model::EnvVar {
            name: "HTTP_PROXY".to_owned(),
            value: "http://user:••••@proxy.corp:8080".to_owned(),
            is_exclusions: false,
        }],
        env_available: true,
    };
    // camelCase 序列化 + 状态标签
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["sources"][0]["id"], "macos-sc");
    assert_eq!(json["sources"][0]["status"]["kind"], "configured");
    assert_eq!(json["sources"][0]["entries"][0]["host"], "proxy.corp");
    assert_eq!(json["sources"][1]["status"]["kind"], "failed");
    assert_eq!(json["sources"][1]["status"]["code"], "proxy.wininet_failed");
    assert_eq!(json["sources"][2]["status"]["kind"], "unsupported");
    assert_eq!(json["envVars"][0]["isExclusions"], false);
    // 反序列化往返
    let back: model::Report = serde_json::from_value(json).unwrap();
    assert_eq!(back.sources.len(), 3);
    assert_eq!(back.sources[0].entries[0].port, 8080);
}

#[test]
fn rejects_unknown_functions() {
    let error = ProxyInspector::default()
        .call("probe", serde_json::json!({}))
        .unwrap_err();
    assert_eq!(error.code, "plugin.function_not_found");
}
