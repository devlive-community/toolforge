use super::*;
use crate::fixtures::{raw, sample};

fn ok(bytes: &[u8]) -> Module {
    analyze(bytes, &|| false).unwrap()
}

#[test]
fn minimal_module_has_only_a_header() {
    let m = ok(&raw(&[]));
    assert_eq!(m.version, Some(1));
    assert!(m.sections.is_empty());
    assert_eq!(m.sizes.total, 8);
    assert_eq!(m.sizes.without_custom, 8);
    assert!(m.parse_error.is_none());
}

#[test]
fn describes_sections_and_sizes() {
    let bytes = sample();
    let m = ok(&bytes);
    let kinds: Vec<&str> = m.sections.iter().map(|s| s.kind.as_str()).collect();
    assert_eq!(
        kinds,
        [
            "type", "import", "function", "table", "global", "export", "code", "custom", "custom",
            "custom"
        ]
    );
    // section 首尾相接，覆盖文件头之后的全部字节
    let mut next = 8;
    for s in &m.sections {
        assert_eq!(s.offset, next);
        next += s.size;
    }
    assert_eq!(next, bytes.len() as u64);
    let customs: Vec<&str> = m
        .sections
        .iter()
        .filter_map(|s| s.name.as_deref())
        .collect();
    assert_eq!(customs, ["name", "producers", "note"]);
    // custom section 不计入代码与模块体积
    let custom: u64 = m
        .sections
        .iter()
        .filter(|s| s.kind == "custom")
        .map(|s| s.size)
        .sum();
    assert_eq!(m.sizes.custom, custom);
    assert!(custom > 100);
    assert_eq!(m.sizes.without_custom, bytes.len() as u64 - custom);
    let code = m.sections.iter().find(|s| s.kind == "code").unwrap();
    assert_eq!(m.sizes.code, code.size);
    assert_eq!(code.items, Some(2));
    assert_eq!(
        m.sizes.header + m.sizes.code + m.sizes.data + m.sizes.custom + m.sizes.other,
        m.sizes.total
    );
}

#[test]
fn lists_imports_exports_and_functions() {
    let m = ok(&sample());
    assert_eq!(m.counts.types, 2);
    assert_eq!(m.counts.imported_functions, 1);
    assert_eq!(m.counts.imported_memories, 1);
    assert_eq!(m.counts.functions, 2);
    assert_eq!(m.counts.tables, 1);
    assert_eq!(m.counts.globals, 1);
    assert_eq!(m.counts.exports, 3);

    assert_eq!(m.imports[0].module, "env");
    assert_eq!(m.imports[0].kind, "func");
    assert_eq!(m.imports[0].desc, "()");
    assert_eq!(m.imports[1].kind, "memory");
    assert_eq!(m.imports[1].desc, "1..2");

    let add = &m.exports[0];
    assert_eq!((add.name.as_str(), add.kind, add.index), ("add", "func", 1));
    assert_eq!(add.desc, "(param i32 i32) (result i32)");
    assert_eq!(m.exports[1].desc, "1.. funcref");
    assert_eq!(m.exports[2].desc, "mut i32");

    // 函数索引空间：导入函数在前
    let f = &m.functions[0];
    assert_eq!(f.index, 1);
    assert_eq!(f.name.as_deref(), Some("add"));
    assert_eq!(f.export.as_deref(), Some("add"));
    assert_eq!(f.signature, "(param i32 i32) (result i32)");
    assert_eq!(f.locals, 2);
    assert!(f.size > 0);
    assert_eq!(m.functions[1].name.as_deref(), Some("run"));
    assert_eq!(m.functions[1].export, None);

    let mem = &m.memories[0];
    assert_eq!(mem.import.as_deref(), Some("env.mem"));
    assert_eq!(
        (mem.min, mem.max, mem.page_size),
        (1, Some(2), Some(PAGE_SIZE))
    );
    assert_eq!(m.tables[0].element.as_deref(), Some("funcref"));
    assert_eq!(m.producers[0].field, "processed-by");
    assert_eq!(m.producers[0].values, ["rustc 1.99.0"]);
}

#[test]
fn rejects_bad_magic_and_components() {
    let err = analyze(b"\0wasm\x01\0\0", &|| false).unwrap_err();
    assert_eq!((err.code.as_str(), err.offset), ("wasm.bad_magic", Some(0)));
    assert_eq!(
        analyze(b"\0as", &|| false).unwrap_err().code,
        "wasm.bad_magic"
    );
    let component = wasm_encoder::Component::new().finish();
    assert_eq!(
        analyze(&component, &|| false).unwrap_err().code,
        "wasm.component"
    );
}

#[test]
fn stops_at_corrupted_bytes_with_offset() {
    // section 长度的 LEB128 在文件末尾被截断
    let m = ok(&raw(&[1, 0x80]));
    let err = m.parse_error.unwrap();
    assert_eq!(err.code, "wasm.parse");
    assert!(err.offset.unwrap() >= 9);
    // 超长的 LEB128（u32 最多 5 个字节）
    let m = ok(&raw(&[1, 0x80, 0x80, 0x80, 0x80, 0x80, 0x00]));
    assert!(m.parse_error.is_some());
    // section 长度超出文件
    let m = ok(&raw(&[1, 20, 0]));
    assert!(m.parse_error.is_some());
    assert!(m.sections.is_empty());
}

#[test]
fn can_be_cancelled() {
    let err = analyze(&sample(), &|| true).unwrap_err();
    assert_eq!(err.code, "task.cancelled");
}
