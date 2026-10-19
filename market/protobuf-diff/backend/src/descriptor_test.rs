use super::*;

/// 测试用描述符编码器（crate::descriptor::build）
use build as b;

fn parse(data: &[u8]) -> PluginResult<FileSet> {
    let mut set = parse_file_set(data)?;
    normalize_types(&mut set);
    validate(&set)?;
    Ok(set)
}

#[test]
fn parses_basic_file() {
    let data = b::file_set(&[b::file(
        "demo.proto",
        "demo",
        &[b::message(
            "User",
            &[
                b::field("id", 1, LABEL_OPTIONAL, FieldType::Int64 as i64, None),
                b::field("name", 2, LABEL_OPTIONAL, FieldType::String as i64, None),
                b::field("tags", 3, LABEL_REPEATED, FieldType::String as i64, None),
            ],
            &[],
            &[],
        )],
        &[],
        &[],
    )]);
    let set = parse(&data).unwrap();
    assert_eq!(set.files.len(), 1);
    let file = &set.files[0];
    assert_eq!(
        (
            file.name.as_str(),
            file.package.as_str(),
            file.syntax.as_str()
        ),
        ("demo.proto", "demo", "proto3")
    );
    assert!(file.unsupported_syntax.is_none());
    let user = &file.messages[0];
    assert_eq!(user.full_name, "demo.User");
    assert_eq!(user.fields.len(), 3);
    assert_eq!(user.fields[0].number, 1);
    assert_eq!(user.fields[0].type_name, "");
    assert_eq!(user.fields[2].label, LABEL_REPEATED);
    // json_name 缺省时按 snake→camel 推导
    assert_eq!(user.fields[0].json_name, "id");
}

#[test]
fn resolves_relative_type_names() {
    // 外层消息内嵌套 Child，字段引用 "Child"（相对名）
    let data = b::file_set(&[b::file(
        "nested.proto",
        "demo",
        &[b::message(
            "Parent",
            &[b::field(
                "child",
                1,
                LABEL_OPTIONAL,
                FieldType::Message as i64,
                Some("Child"),
            )],
            &[b::message(
                "Child",
                &[b::field(
                    "id",
                    1,
                    LABEL_OPTIONAL,
                    FieldType::Int32 as i64,
                    None,
                )],
                &[],
                &[],
            )],
            &[],
        )],
        &[],
        &[],
    )]);
    let set = parse(&data).unwrap();
    let parent = &set.files[0].messages[0];
    assert_eq!(parent.full_name, "demo.Parent");
    assert_eq!(parent.fields[0].type_name, "demo.Parent.Child");
    let child = &parent.nested[0];
    assert_eq!(child.full_name, "demo.Parent.Child");
}

#[test]
fn proto2_and_editions_marked_unsupported() {
    let set = parse_file_set(&b::file_set(&[b::file_with_syntax(
        "legacy.proto",
        "",
        "proto2",
    )]))
    .unwrap();
    assert_eq!(set.files[0].unsupported_syntax.as_deref(), Some("proto2"));
    // editions 用字段 14
    let mut out = Vec::new();
    b::encode_string_field(1, "ed.proto", &mut out);
    b::encode_string_field(14, "2023", &mut out);
    let set = parse_file_set(&b::file_set(&[out])).unwrap();
    assert_eq!(
        set.files[0].unsupported_syntax.as_deref(),
        Some("edition:2023")
    );
}

#[test]
fn missing_dependency_fails() {
    let data = b::file_set(&[b::file_with_deps(
        "app.proto",
        "",
        &[],
        &[],
        &[],
        &["missing.proto"],
    )]);
    let mut set = parse_file_set(&data).unwrap();
    normalize_types(&mut set);
    let err = validate(&set).unwrap_err();
    assert_eq!(err.code, "pbd.missing_dependency");
    assert!(
        err.params["files"]
            .as_str()
            .unwrap()
            .contains("missing.proto")
    );
}

#[test]
fn duplicate_symbol_fails() {
    let data = b::file_set(&[
        b::file(
            "a.proto",
            "pkg",
            &[b::message("Dup", &[], &[], &[])],
            &[],
            &[],
        ),
        b::file(
            "b.proto",
            "pkg",
            &[b::message("Dup", &[], &[], &[])],
            &[],
            &[],
        ),
    ]);
    let mut set = parse_file_set(&data).unwrap();
    normalize_types(&mut set);
    let err = validate(&set).unwrap_err();
    assert_eq!(err.code, "pbd.duplicate_symbol");
}

#[test]
fn unknown_type_reference_fails() {
    let data = b::file_set(&[b::file(
        "bad.proto",
        "",
        &[b::message(
            "M",
            &[b::field(
                "x",
                1,
                LABEL_OPTIONAL,
                FieldType::Message as i64,
                Some(".no.Such"),
            )],
            &[],
            &[],
        )],
        &[],
        &[],
    )]);
    let mut set = parse_file_set(&data).unwrap();
    normalize_types(&mut set);
    let err = validate(&set).unwrap_err();
    assert_eq!(err.code, "pbd.unknown_type");
}

#[test]
fn oneof_and_optional_markers() {
    // oneof_decl "choice"（字段 8）；字段 email 带 oneof_index 0
    let email = b::with_oneof_index(
        b::field("email", 1, LABEL_OPTIONAL, FieldType::String as i64, None),
        0,
    );
    let nickname = b::proto3_optional(b::field(
        "nickname",
        2,
        LABEL_OPTIONAL,
        FieldType::String as i64,
        None,
    ));
    let msg = b::message_with_oneofs(
        "Contact",
        &[email, b::with_oneof_index(nickname, 1)],
        &["choice", "_nickname"],
    );
    let data = b::file_set(&[b::file("c.proto", "", &[msg], &[], &[])]);
    let set = parse(&data).unwrap();
    let contact = &set.files[0].messages[0];
    assert_eq!(contact.oneofs, vec!["choice", "_nickname"]);
    assert_eq!(contact.fields[0].oneof, "choice");
    assert!(!contact.fields[0].proto3_optional);
    assert!(contact.fields[1].proto3_optional);
}

#[test]
fn source_locations_and_comments() {
    // FileDescriptorProto.source_code_info(9) → Location{path:[4,0,2,1], span:[0,0,3,1], leading_comments:" id\n"}
    let mut location = Vec::new();
    let mut path = Vec::new();
    b::encode_varint(4, &mut path);
    b::encode_varint(0, &mut path);
    b::encode_varint(2, &mut path);
    b::encode_varint(1, &mut path);
    b::encode_len_field(1, &path, &mut location);
    let mut span = Vec::new();
    for value in [0i64, 0, 3, 1] {
        b::encode_varint(value as u64, &mut span);
    }
    b::encode_len_field(2, &span, &mut location);
    b::encode_string_field(3, " id\n", &mut location);
    let mut file_bytes = Vec::new();
    b::encode_string_field(1, "loc.proto", &mut file_bytes);
    b::encode_string_field(2, "pkg", &mut file_bytes);
    b::encode_len_field(4, &b::message("M", &[], &[], &[]), &mut file_bytes);
    let mut source_info = Vec::new();
    b::encode_len_field(1, &location, &mut source_info);
    b::encode_len_field(9, &source_info, &mut file_bytes);
    b::encode_string_field(12, "proto3", &mut file_bytes);
    let set = parse_file_set(&b::file_set(&[file_bytes])).unwrap();
    let locations = &set.files[0].locations;
    assert_eq!(locations.len(), 1);
    assert_eq!(locations[0].path, vec![4, 0, 2, 1]);
    assert_eq!(locations[0].span, vec![0, 0, 3, 1]);
    assert_eq!(locations[0].leading_comments, " id\n");
}

#[test]
fn limits_are_enforced() {
    // 超过文件数上限
    let files: Vec<Vec<u8>> = (0..300)
        .map(|i| b::file(&format!("f{i}.proto"), "", &[], &[], &[]))
        .collect();
    let data = b::file_set(&files);
    assert_eq!(
        parse_file_set(&data).unwrap_err().code,
        "pbd.too_many_files"
    );
    // 空输入合法：空集合
    assert!(parse(&[]).unwrap().files.is_empty());
}
