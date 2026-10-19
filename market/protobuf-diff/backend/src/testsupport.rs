//! 测试专用：把结构化描述符定义编码为 FileDescriptorSet wire bytes。
//! 仅在 cfg(test) 下编译，模拟 protoc 产物。

pub fn encode_varint(mut value: u64, out: &mut Vec<u8>) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            break;
        }
        out.push(byte | 0x80);
    }
}

pub fn encode_key(number: u32, wire_type: u8, out: &mut Vec<u8>) {
    encode_varint((u64::from(number) << 3) | u64::from(wire_type), out);
}

pub fn encode_varint_field(number: u32, value: u64, out: &mut Vec<u8>) {
    encode_key(number, 0, out);
    encode_varint(value, out);
}

pub fn encode_len_field(number: u32, payload: &[u8], out: &mut Vec<u8>) {
    encode_key(number, 2, out);
    encode_varint(payload.len() as u64, out);
    out.extend_from_slice(payload);
}

pub fn encode_string_field(number: u32, text: &str, out: &mut Vec<u8>) {
    encode_len_field(number, text.as_bytes(), out);
}

pub fn zigzag_encode(value: i64) -> u64 {
    ((value << 1) ^ (value >> 63)) as u64
}

pub fn field(
    name: &str,
    number: i64,
    label: i64,
    type_code: i64,
    type_name: Option<&str>,
) -> Vec<u8> {
    let mut out = Vec::new();
    encode_string_field(1, name, &mut out);
    encode_varint_field(3, number as u64, &mut out);
    encode_varint_field(4, label as u64, &mut out);
    encode_varint_field(5, type_code as u64, &mut out);
    if let Some(type_name) = type_name {
        encode_string_field(6, type_name, &mut out);
    }
    out
}

/// 附加 proto3_optional 标记（字段 17）
pub fn proto3_optional(mut field_bytes: Vec<u8>) -> Vec<u8> {
    encode_varint_field(17, 1, &mut field_bytes);
    field_bytes
}

pub fn message(
    name: &str,
    fields: &[Vec<u8>],
    nested: &[Vec<u8>],
    reserved_ranges: &[(i64, i64)],
) -> Vec<u8> {
    message_full(name, fields, nested, &[], reserved_ranges, &[])
}

#[allow(clippy::too_many_arguments)]
pub fn message_full(
    name: &str,
    fields: &[Vec<u8>],
    nested: &[Vec<u8>],
    enums: &[Vec<u8>],
    reserved_ranges: &[(i64, i64)],
    reserved_names: &[&str],
    // oneofs: 名称列表；fields 里通过 oneof_index 引用
) -> Vec<u8> {
    let _ = reserved_names;
    let mut out = Vec::new();
    encode_string_field(1, name, &mut out);
    for field in fields {
        encode_len_field(2, field, &mut out);
    }
    for nested_message in nested {
        encode_len_field(3, nested_message, &mut out);
    }
    for enum_desc in enums {
        encode_len_field(4, enum_desc, &mut out);
    }
    for (start, end) in reserved_ranges {
        let mut range = Vec::new();
        encode_varint_field(1, *start as u64, &mut range);
        encode_varint_field(2, *end as u64, &mut range);
        encode_len_field(9, &range, &mut out);
    }
    for reserved in reserved_names {
        encode_string_field(10, reserved, &mut out);
    }
    out
}

/// 带 oneof_decl 的消息
pub fn message_with_oneofs(name: &str, fields: &[Vec<u8>], oneofs: &[&str]) -> Vec<u8> {
    let mut out = Vec::new();
    encode_string_field(1, name, &mut out);
    for field in fields {
        encode_len_field(2, field, &mut out);
    }
    for oneof in oneofs {
        encode_string_field(8, oneof, &mut out);
    }
    out
}

/// 字段的 oneof_index 引用
pub fn with_oneof_index(mut field_bytes: Vec<u8>, index: i64) -> Vec<u8> {
    encode_varint_field(9, index as u64, &mut field_bytes);
    field_bytes
}

pub fn enum_value(name: &str, number: i64) -> Vec<u8> {
    let mut out = Vec::new();
    encode_string_field(1, name, &mut out);
    encode_varint_field(2, number as u64, &mut out);
    out
}

pub fn enum_desc(name: &str, values: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    encode_string_field(1, name, &mut out);
    for value in values {
        encode_len_field(2, value, &mut out);
    }
    out
}

/// 枚举 reserved：区间（4）与名称（5）
pub fn enum_reserved(
    name: &str,
    values: &[Vec<u8>],
    ranges: &[(i64, i64)],
    names: &[&str],
) -> Vec<u8> {
    let mut out = enum_desc(name, values);
    for (start, end) in ranges {
        let mut range = Vec::new();
        encode_varint_field(1, *start as u64, &mut range);
        encode_varint_field(2, *end as u64, &mut range);
        encode_len_field(4, &range, &mut out);
    }
    for reserved in names {
        encode_string_field(5, reserved, &mut out);
    }
    out
}

pub fn service(name: &str, methods: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    encode_string_field(1, name, &mut out);
    for method in methods {
        encode_len_field(2, method, &mut out);
    }
    out
}

pub fn method(
    name: &str,
    input: &str,
    output: &str,
    client_streaming: bool,
    server_streaming: bool,
) -> Vec<u8> {
    let mut out = Vec::new();
    encode_string_field(1, name, &mut out);
    encode_string_field(2, input, &mut out);
    encode_string_field(3, output, &mut out);
    if client_streaming {
        encode_varint_field(5, 1, &mut out);
    }
    if server_streaming {
        encode_varint_field(6, 1, &mut out);
    }
    out
}

pub fn file(
    name: &str,
    package: &str,
    messages: &[Vec<u8>],
    enums: &[Vec<u8>],
    services: &[Vec<u8>],
) -> Vec<u8> {
    file_with_deps(name, package, messages, enums, services, &[])
}

pub fn file_with_deps(
    name: &str,
    package: &str,
    messages: &[Vec<u8>],
    enums: &[Vec<u8>],
    services: &[Vec<u8>],
    dependencies: &[&str],
) -> Vec<u8> {
    let mut out = Vec::new();
    encode_string_field(1, name, &mut out);
    encode_string_field(2, package, &mut out);
    for dep in dependencies {
        encode_string_field(3, dep, &mut out);
    }
    for message in messages {
        encode_len_field(4, message, &mut out);
    }
    for enum_desc in enums {
        encode_len_field(5, enum_desc, &mut out);
    }
    for service_def in services {
        encode_len_field(6, service_def, &mut out);
    }
    encode_string_field(12, "proto3", &mut out);
    out
}

pub fn file_with_syntax(name: &str, package: &str, syntax: &str) -> Vec<u8> {
    let mut out = Vec::new();
    encode_string_field(1, name, &mut out);
    encode_string_field(2, package, &mut out);
    encode_string_field(12, syntax, &mut out);
    out
}

pub fn file_set(files: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    for file in files {
        encode_len_field(1, file, &mut out);
    }
    out
}
