use super::*;
use crate::descriptor::{FieldType, LABEL_OPTIONAL, LABEL_REPEATED, build as b};

fn load(data: &[u8]) -> FileSet {
    let mut set = crate::descriptor::parse_file_set(data).unwrap();
    crate::descriptor::normalize_types(&mut set);
    crate::descriptor::validate(&set).unwrap();
    set
}

fn findings<'a>(report: &'a Report, rule: &str) -> Vec<&'a Finding> {
    report.findings.iter().filter(|f| f.rule == rule).collect()
}

/// 基线消息：id(int64) + name(string)
fn user_message() -> Vec<u8> {
    b::message(
        "User",
        &[
            b::field("id", 1, LABEL_OPTIONAL, FieldType::Int64 as i64, None),
            b::field("name", 2, LABEL_OPTIONAL, FieldType::String as i64, None),
        ],
        &[],
        &[],
    )
}

fn demo_file(messages: &[Vec<u8>]) -> FileSet {
    load(&b::file_set(&[b::file(
        "demo.proto",
        "demo",
        messages,
        &[],
        &[],
    )]))
}

fn load_file(file: Vec<u8>) -> FileSet {
    load(&b::file_set(&[file]))
}

#[test]
fn identical_sets_produce_no_findings() {
    let report = compare(&demo_file(&[user_message()]), &demo_file(&[user_message()]));
    assert!(report.findings.is_empty());
}

#[test]
fn field_order_and_comments_do_not_trigger() {
    // 顺序颠倒 + 无注释：同字段号同类型不报（注释/顺序不参与）
    let reordered = b::message(
        "User",
        &[
            b::field("name", 2, LABEL_OPTIONAL, FieldType::String as i64, None),
            b::field("id", 1, LABEL_OPTIONAL, FieldType::Int64 as i64, None),
        ],
        &[],
        &[],
    );
    let report = compare(&demo_file(&[user_message()]), &demo_file(&[reordered]));
    assert!(report.findings.is_empty());
}

#[test]
fn renumbered_field_reports_removal_and_addition() {
    // 字段重编号：旧 1 号消失（未保留）+ 新 3 号出现
    let renumbered = b::message(
        "User",
        &[
            b::field("id", 3, LABEL_OPTIONAL, FieldType::Int64 as i64, None),
            b::field("name", 2, LABEL_OPTIONAL, FieldType::String as i64, None),
        ],
        &[],
        &[],
    );
    let report = compare(&demo_file(&[user_message()]), &demo_file(&[renumbered]));
    let removed = findings(&report, "field.removed");
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].path, "demo.User.id");
    assert_eq!(removed[0].severity, Severity::Breaking);
    let added = findings(&report, "field.added");
    assert_eq!(added.len(), 1);
    assert_eq!(added[0].path, "demo.User.id");
}

#[test]
fn removed_field_with_reserve_is_safe() {
    // 删除字段且 reserved 覆盖旧号：info
    let updated = b::message_full(
        "User",
        &[b::field(
            "name",
            2,
            LABEL_OPTIONAL,
            FieldType::String as i64,
            None,
        )],
        &[],
        &[],
        &[(1, 2)],
        &[],
    );
    let report = compare(&demo_file(&[user_message()]), &demo_file(&[updated]));
    let removed = findings(&report, "field.removed_reserved");
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].severity, Severity::Info);
    assert_eq!(removed[0].new_desc, "reserved");
}

#[test]
fn old_reserved_reused_by_new_field_is_breaking() {
    // 旧 reserved 区间被新字段占用：明确破坏
    let old = demo_file(&[b::message_full("User", &[], &[], &[], &[(5, 10)], &[])]);
    let new = demo_file(&[b::message_full(
        "User",
        &[b::field(
            "sneaky",
            6,
            LABEL_OPTIONAL,
            FieldType::String as i64,
            None,
        )],
        &[],
        &[],
        &[],
        &[],
    )]);
    let report = compare(&old, &new);
    let reused = findings(&report, "reserved.reused");
    assert_eq!(reused.len(), 1);
    assert_eq!(reused[0].severity, Severity::Breaking);
    assert_eq!(reused[0].old_desc, "reserved 6");
}

#[test]
fn rename_same_number_is_protojson_only() {
    // 同字段号改名：二进制兼容，ProtoJSON 名称风险；不断言号码复用
    let renamed = b::message(
        "User",
        &[
            b::field("id", 1, LABEL_OPTIONAL, FieldType::Int64 as i64, None),
            b::field(
                "full_name",
                2,
                LABEL_OPTIONAL,
                FieldType::String as i64,
                None,
            ),
        ],
        &[],
        &[],
    );
    let report = compare(&demo_file(&[user_message()]), &demo_file(&[renamed]));
    let renamed_findings = findings(&report, "field.renamed");
    assert_eq!(renamed_findings.len(), 1);
    assert_eq!(renamed_findings[0].direction, Direction::ProtoJson);
    assert_eq!(renamed_findings[0].old_desc, "name");
    assert_eq!(renamed_findings[0].new_desc, "full_name");
    // 无 wire 破坏
    assert!(findings(&report, "field.wire_type_changed").is_empty());
}

#[test]
fn wire_class_change_is_breaking() {
    // int64 → string：wire class 变化，双向破坏
    let changed = b::message(
        "User",
        &[
            b::field("id", 1, LABEL_OPTIONAL, FieldType::String as i64, None),
            b::field("name", 2, LABEL_OPTIONAL, FieldType::String as i64, None),
        ],
        &[],
        &[],
    );
    let report = compare(&demo_file(&[user_message()]), &demo_file(&[changed]));
    let changed_findings = findings(&report, "field.wire_type_changed");
    assert_eq!(changed_findings.len(), 1);
    assert_eq!(changed_findings[0].severity, Severity::Breaking);
    assert_eq!(changed_findings[0].direction, Direction::Both);
    assert_eq!(changed_findings[0].old_desc, "int64");
    assert_eq!(changed_findings[0].new_desc, "string");
}

#[test]
fn varint_semantics_are_conditional_with_json_number_form() {
    // int64 → int32：同 varint 组，条件性（范围截断）+ ProtoJSON 数字表示变化
    let changed = b::message(
        "User",
        &[
            b::field("id", 1, LABEL_OPTIONAL, FieldType::Int32 as i64, None),
            b::field("name", 2, LABEL_OPTIONAL, FieldType::String as i64, None),
        ],
        &[],
        &[],
    );
    let report = compare(&demo_file(&[user_message()]), &demo_file(&[changed]));
    assert_eq!(findings(&report, "field.varint_semantics").len(), 1);
    assert_eq!(findings(&report, "field.json_number_form").len(), 1);
    assert_eq!(
        findings(&report, "field.json_number_form")[0].direction,
        Direction::ProtoJson
    );
}

#[test]
fn zigzag_change_is_breaking() {
    // int64 → sint64：zigzag 与普通 varint 数值解释不同
    let changed = b::message(
        "User",
        &[
            b::field("id", 1, LABEL_OPTIONAL, FieldType::Sint64 as i64, None),
            b::field("name", 2, LABEL_OPTIONAL, FieldType::String as i64, None),
        ],
        &[],
        &[],
    );
    let report = compare(&demo_file(&[user_message()]), &demo_file(&[changed]));
    let zigzag = findings(&report, "field.zigzag_changed");
    assert_eq!(zigzag.len(), 1);
    assert_eq!(zigzag[0].severity, Severity::Breaking);
}

#[test]
fn float_reinterpret_is_breaking() {
    // fixed64 → double：位解释改变
    let changed = demo_file(&[b::message(
        "User",
        &[b::field(
            "amount",
            1,
            LABEL_OPTIONAL,
            FieldType::Double as i64,
            None,
        )],
        &[],
        &[],
    )]);
    let old = demo_file(&[b::message(
        "User",
        &[b::field(
            "amount",
            1,
            LABEL_OPTIONAL,
            FieldType::Fixed64 as i64,
            None,
        )],
        &[],
        &[],
    )]);
    let report = compare(&old, &changed);
    let reinterpreted = findings(&report, "field.float_reinterpret");
    assert_eq!(reinterpreted.len(), 1);
    assert_eq!(reinterpreted[0].severity, Severity::Breaking);
    // fixed64 ↔ sfixed64：同组但符号语义不同，条件性
    let signed = demo_file(&[b::message(
        "User",
        &[b::field(
            "amount",
            1,
            LABEL_OPTIONAL,
            FieldType::Sfixed64 as i64,
            None,
        )],
        &[],
        &[],
    )]);
    let report = compare(&old, &signed);
    let conditional = findings(&report, "field.int64_semantics");
    assert_eq!(conditional.len(), 1);
    assert_eq!(conditional[0].severity, Severity::Conditional);
}

#[test]
fn string_bytes_is_conditional() {
    let old = demo_file(&[b::message(
        "Blob",
        &[b::field(
            "data",
            1,
            LABEL_OPTIONAL,
            FieldType::String as i64,
            None,
        )],
        &[],
        &[],
    )]);
    let new = demo_file(&[b::message(
        "Blob",
        &[b::field(
            "data",
            1,
            LABEL_OPTIONAL,
            FieldType::Bytes as i64,
            None,
        )],
        &[],
        &[],
    )]);
    let report = compare(&old, &new);
    let found = findings(&report, "field.string_bytes");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].severity, Severity::Conditional);
}

#[test]
fn message_retype_is_conditional() {
    let old = demo_file(&[b::message(
        "Holder",
        &[b::field(
            "payload",
            1,
            LABEL_OPTIONAL,
            FieldType::Message as i64,
            Some(".demo.Holder.A"),
        )],
        &[b::message(
            "A",
            &[b::field(
                "x",
                1,
                LABEL_OPTIONAL,
                FieldType::Int32 as i64,
                None,
            )],
            &[],
            &[],
        )],
        &[],
    )]);
    let new = demo_file(&[b::message(
        "Holder",
        &[b::field(
            "payload",
            1,
            LABEL_OPTIONAL,
            FieldType::Message as i64,
            Some(".demo.Holder.B"),
        )],
        &[b::message(
            "B",
            &[b::field(
                "y",
                1,
                LABEL_OPTIONAL,
                FieldType::Int32 as i64,
                None,
            )],
            &[],
            &[],
        )],
        &[],
    )]);
    let report = compare(&old, &new);
    let retyped = findings(&report, "field.message_retyped");
    assert_eq!(retyped.len(), 1);
    assert_eq!(retyped[0].old_desc, "demo.Holder.A");
    assert_eq!(retyped[0].new_desc, "demo.Holder.B");
}

#[test]
fn cardinality_changes() {
    // singular → repeated：条件性（值语义变化）
    let widened = demo_file(&[b::message(
        "User",
        &[
            b::field("id", 1, LABEL_OPTIONAL, FieldType::Int64 as i64, None),
            b::field("name", 2, LABEL_REPEATED, FieldType::String as i64, None),
        ],
        &[],
        &[],
    )]);
    let report = compare(&demo_file(&[user_message()]), &widened);
    assert_eq!(findings(&report, "field.cardinality_widened").len(), 1);
    // repeated → singular：破坏（多元素丢失）
    let narrowed = demo_file(&[b::message(
        "Tagged",
        &[b::field(
            "tags",
            1,
            LABEL_OPTIONAL,
            FieldType::String as i64,
            None,
        )],
        &[],
        &[],
    )]);
    let base = demo_file(&[b::message(
        "Tagged",
        &[b::field(
            "tags",
            1,
            LABEL_REPEATED,
            FieldType::String as i64,
            None,
        )],
        &[],
        &[],
    )]);
    let report = compare(&base, &narrowed);
    let narrowed_findings = findings(&report, "field.cardinality_narrowed");
    assert_eq!(narrowed_findings.len(), 1);
    assert_eq!(narrowed_findings[0].severity, Severity::Breaking);
}

#[test]
fn presence_and_oneof_changes() {
    // proto3 optional 翻转：info
    let optional = b::proto3_optional(b::field(
        "id",
        1,
        LABEL_OPTIONAL,
        FieldType::Int64 as i64,
        None,
    ));
    let old = load(&b::file_set(&[b::file(
        "p.proto",
        "",
        &[b::message(
            "M",
            &[b::field(
                "id",
                1,
                LABEL_OPTIONAL,
                FieldType::Int64 as i64,
                None,
            )],
            &[],
            &[],
        )],
        &[],
        &[],
    )]));
    let new = load(&b::file_set(&[b::file(
        "p.proto",
        "",
        &[b::message("M", &[optional], &[], &[])],
        &[],
        &[],
    )]));
    let report = compare(&old, &new);
    assert_eq!(findings(&report, "field.presence_changed").len(), 1);
    assert_eq!(
        findings(&report, "field.presence_changed")[0].severity,
        Severity::Info
    );
    // oneof 归属变化：info
    let old = load(&b::file_set(&[b::file(
        "o.proto",
        "",
        &[b::message(
            "M",
            &[b::field(
                "choice",
                1,
                LABEL_OPTIONAL,
                FieldType::Int32 as i64,
                None,
            )],
            &[],
            &[],
        )],
        &[],
        &[],
    )]));
    let new = load(&b::file_set(&[b::file(
        "o.proto",
        "",
        &[b::message_with_oneofs(
            "M",
            &[b::with_oneof_index(
                b::field("choice", 1, LABEL_OPTIONAL, FieldType::Int32 as i64, None),
                0,
            )],
            &["pick"],
        )],
        &[],
        &[],
    )]));
    let report = compare(&old, &new);
    let oneof = findings(&report, "field.oneof_changed");
    assert_eq!(oneof.len(), 1);
    assert_eq!(oneof[0].new_desc, "pick");
}

#[test]
fn enum_value_rules() {
    // 旧值删除未保留：conditional；新值占用旧 reserved：breaking；改名：protojson
    let old = load_file(b::file(
        "demo.proto",
        "demo",
        &[b::message(
            "M",
            &[b::field(
                "status",
                1,
                LABEL_OPTIONAL,
                FieldType::Enum as i64,
                Some(".demo.Status"),
            )],
            &[],
            &[],
        )],
        &[b::enum_reserved(
            "Status",
            &[b::enum_value("OPEN", 0), b::enum_value("CLOSED", 1)],
            &[],
            &[],
        )],
        &[],
    ));
    let report = compare(&old, &old);
    assert!(report.findings.is_empty());
    // 值改名
    let renamed_enum = load_file(b::file(
        "demo.proto",
        "demo",
        &[b::message(
            "M",
            &[b::field(
                "status",
                1,
                LABEL_OPTIONAL,
                FieldType::Enum as i64,
                Some(".demo.Status"),
            )],
            &[],
            &[],
        )],
        &[b::enum_reserved(
            "Status",
            &[b::enum_value("OPEN", 0), b::enum_value("SHUT", 1)],
            &[],
            &[],
        )],
        &[],
    ));
    let report = compare(&old, &renamed_enum);
    let renamed = findings(&report, "enum.value_renamed");
    assert_eq!(renamed.len(), 1);
    assert_eq!(renamed[0].direction, Direction::ProtoJson);
    // 值删除未保留：conditional + reserved 覆盖：info
    let removed = load_file(b::file(
        "demo.proto",
        "demo",
        &[b::message(
            "M",
            &[b::field(
                "status",
                1,
                LABEL_OPTIONAL,
                FieldType::Enum as i64,
                Some(".demo.Status"),
            )],
            &[],
            &[],
        )],
        &[b::enum_reserved(
            "Status",
            &[b::enum_value("OPEN", 0)],
            &[],
            &[],
        )],
        &[],
    ));
    let report = compare(&old, &removed);
    assert_eq!(findings(&report, "enum.value_removed").len(), 1);
    // 新值占用旧 reserved
    let reused = load_file(b::file(
        "demo.proto",
        "demo",
        &[b::message(
            "M",
            &[b::field(
                "status",
                1,
                LABEL_OPTIONAL,
                FieldType::Enum as i64,
                Some(".demo.Status"),
            )],
            &[],
            &[],
        )],
        &[b::enum_reserved(
            "Status",
            &[
                b::enum_value("OPEN", 0),
                b::enum_value("CLOSED", 1),
                b::enum_value("GHOST", 2),
            ],
            &[],
            &[],
        )],
        &[],
    ));
    let reserved_old = load_file(b::file(
        "demo.proto",
        "demo",
        &[b::message(
            "M",
            &[b::field(
                "status",
                1,
                LABEL_OPTIONAL,
                FieldType::Enum as i64,
                Some(".demo.Status"),
            )],
            &[],
            &[],
        )],
        &[b::enum_reserved(
            "Status",
            &[b::enum_value("OPEN", 0)],
            &[(2, 3)],
            &[],
        )],
        &[],
    ));
    let report = compare(&reserved_old, &reused);
    let reused_findings = findings(&report, "reserved.reused");
    assert_eq!(reused_findings.len(), 1);
    assert_eq!(reused_findings[0].severity, Severity::Breaking);
}

#[test]
fn service_rules() {
    let old = load_file(b::file(
        "demo.proto",
        "demo",
        &[],
        &[],
        &[b::service(
            "Svc",
            &[b::method("Get", ".demo.User", ".demo.User", false, false)],
        )],
    ));
    let report = compare(&old, &old);
    assert!(report.findings.is_empty());
    // 输入类型变化 + 流式变化 → breaking；新方法 → info
    let new = load_file(b::file(
        "demo.proto",
        "demo",
        &[b::message(
            "Summary",
            &[b::field(
                "count",
                1,
                LABEL_OPTIONAL,
                FieldType::Int32 as i64,
                None,
            )],
            &[],
            &[],
        )],
        &[],
        &[b::service(
            "Svc",
            &[
                b::method("Get", ".demo.Summary", ".demo.User", false, true),
                b::method("List", ".demo.User", ".demo.User", false, false),
            ],
        )],
    ));
    let report = compare(&old, &new);
    assert_eq!(findings(&report, "method.input_changed").len(), 1);
    assert_eq!(findings(&report, "method.streaming_changed").len(), 1);
    assert_eq!(findings(&report, "method.added").len(), 1);
    // 方法删除 → breaking
    let removed = load_file(b::file(
        "demo.proto",
        "demo",
        &[],
        &[],
        &[b::service("Svc", &[])],
    ));
    let report = compare(&old, &removed);
    assert_eq!(findings(&report, "method.removed").len(), 1);
    assert_eq!(
        findings(&report, "method.removed")[0].severity,
        Severity::Breaking
    );
}

#[test]
fn file_level_rules() {
    // 文件删除 → breaking；新增 → info（不同文件名、不同消息避免重复符号）
    let old = load(&b::file_set(&[
        b::file("a.proto", "demo", &[user_message()], &[], &[]),
        b::file(
            "b.proto",
            "demo",
            &[b::message(
                "Other",
                &[b::field(
                    "n",
                    1,
                    LABEL_OPTIONAL,
                    FieldType::Int32 as i64,
                    None,
                )],
                &[],
                &[],
            )],
            &[],
            &[],
        ),
    ]));
    let new = load(&b::file_set(&[b::file(
        "b.proto",
        "demo",
        &[b::message(
            "Other",
            &[b::field(
                "n",
                1,
                LABEL_OPTIONAL,
                FieldType::Int32 as i64,
                None,
            )],
            &[],
            &[],
        )],
        &[],
        &[],
    )]));
    let report = compare(&old, &new);
    assert_eq!(findings(&report, "file.removed").len(), 1);
    assert_eq!(
        findings(&report, "file.removed")[0].severity,
        Severity::Breaking
    );
    let report = compare(&new, &old);
    assert_eq!(findings(&report, "file.added").len(), 1);
}

#[test]
fn unsupported_files_excluded_but_listed() {
    // proto2 文件不参与结论：删除不报 file.removed，只进 unsupported 列表
    let old_set = load_with_unsupported("legacy.proto", "proto2");
    let report = compare(&old_set, &demo_file(&[]));
    assert_eq!(report.unsupported.len(), 1);
    assert_eq!(report.unsupported[0].name, "legacy.proto");
    assert!(findings(&report, "file.removed").is_empty());
    // summary 不计入
    assert_eq!(report.old_summary.files, 0);
}

fn load_with_unsupported(name: &str, syntax: &str) -> FileSet {
    // unsupported 文件不参与 validate（这里直接手工构造）
    crate::descriptor::parse_file_set(&b::file_set(&[b::file_with_syntax(name, "", syntax)]))
        .unwrap()
}

#[test]
fn json_name_change_detected() {
    // 同名但显式 json_name 变化：protojson
    let mut old_field = b::field("user_id", 1, LABEL_OPTIONAL, FieldType::Int64 as i64, None);
    b::encode_string_field(10, "userId", &mut old_field);
    let mut new_field = b::field("user_id", 1, LABEL_OPTIONAL, FieldType::Int64 as i64, None);
    b::encode_string_field(10, "userIdentifier", &mut new_field);
    let old = load(&b::file_set(&[b::file(
        "j.proto",
        "",
        &[b::message("M", &[old_field], &[], &[])],
        &[],
        &[],
    )]));
    let new = load(&b::file_set(&[b::file(
        "j.proto",
        "",
        &[b::message("M", &[new_field], &[], &[])],
        &[],
        &[],
    )]));
    let report = compare(&old, &new);
    let json_changed = findings(&report, "field.json_name_changed");
    assert_eq!(json_changed.len(), 1);
    assert_eq!(json_changed[0].direction, Direction::ProtoJson);
}

#[test]
fn findings_sorted_by_severity_and_path() {
    let old = demo_file(&[user_message()]);
    let broken = demo_file(&[b::message(
        "User",
        &[
            b::field("id", 1, LABEL_OPTIONAL, FieldType::String as i64, None),
            b::field("name", 2, LABEL_OPTIONAL, FieldType::String as i64, None),
            b::field("extra", 9, LABEL_OPTIONAL, FieldType::Bool as i64, None),
        ],
        &[],
        &[],
    )]);
    let report = compare(&old, &broken);
    let severities: Vec<Severity> = report.findings.iter().map(|f| f.severity).collect();
    let mut sorted = severities.clone();
    sorted.sort();
    assert_eq!(severities, sorted);
}
