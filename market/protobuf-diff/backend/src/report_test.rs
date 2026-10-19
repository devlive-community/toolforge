use super::*;
use crate::diff::RULES_VERSION;

#[test]
fn export_roundtrip() {
    let report = Report {
        rules_version: RULES_VERSION.to_owned(),
        old_summary: crate::diff::SetSummary {
            files: 1,
            messages: 1,
            enums: 0,
            services: 0,
        },
        new_summary: crate::diff::SetSummary {
            files: 1,
            messages: 1,
            enums: 0,
            services: 0,
        },
        unsupported: vec![],
        findings: vec![Finding {
            severity: Severity::Breaking,
            direction: Direction::Both,
            rule: "field.wire_type_changed".to_owned(),
            path: "demo.User.id".to_owned(),
            old_desc: "int64".to_owned(),
            new_desc: "string".to_owned(),
            location: "demo.proto:3:2".to_owned(),
        }],
        notes: vec!["source-compatibility".to_owned()],
    };
    let out = export(ExportArgs {
        format: "markdown".to_owned(),
        report,
    })
    .unwrap();
    assert_eq!(out.ext, "md");
    assert!(out.content.contains("# Protobuf Compatibility Report"));
    assert!(out.content.contains("field.wire_type_changed"));
    assert!(out.content.contains("demo.User.id"));
    assert!(out.content.contains("int64 → string"));
    assert!(out.content.contains("Not checked"));
}

#[test]
fn unknown_format_rejected() {
    let report = Report {
        rules_version: "1".to_owned(),
        old_summary: crate::diff::SetSummary {
            files: 0,
            messages: 0,
            enums: 0,
            services: 0,
        },
        new_summary: crate::diff::SetSummary {
            files: 0,
            messages: 0,
            enums: 0,
            services: 0,
        },
        unsupported: vec![],
        findings: vec![],
        notes: vec![],
    };
    let err = export(ExportArgs {
        format: "yaml".to_owned(),
        report,
    })
    .unwrap_err();
    assert_eq!(err.code, "pbd.unknown_format");
}
