use super::*;
use crate::fixtures::{raw, sample};
use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function, FunctionSection, MemorySection, MemoryType,
    Module, TypeSection, ValType,
};

fn run(bytes: &[u8]) -> Validation {
    validate(bytes, &|| false).unwrap()
}

#[test]
fn accepts_valid_modules() {
    let v = run(&sample());
    assert_eq!(v.status, "valid");
    assert!(v.error.is_none());
    assert!(v.features.iter().any(|f| f == "simd"));
    assert!(
        !v.features
            .iter()
            .any(|f| f.starts_with("cm-") || f == "component-model")
    );
    assert_eq!(run(&raw(&[])).status, "valid");
}

#[test]
fn rejects_out_of_order_and_duplicate_sections() {
    // function section（3）出现在 type section（1）之前
    let v = run(&raw(&[3, 1, 0, 1, 1, 0]));
    assert_eq!(v.status, "invalid");
    assert!(v.error.unwrap().offset.is_some());
    // 两个 type section
    assert_eq!(run(&raw(&[1, 1, 0, 1, 1, 0])).status, "invalid");
    // 同名 custom section 可以重复
    let custom = [0, 2, 1, b'x'];
    let bytes = raw(&[&custom[..], &custom[..]].concat());
    assert_eq!(run(&bytes).status, "valid");
}

#[test]
fn rejects_out_of_bounds_indices_and_type_mismatches() {
    let mut exports = ExportSection::new();
    exports.export("f", ExportKind::Func, 5);
    let mut module = Module::new();
    module.section(&exports);
    let v = run(&module.finish());
    assert_eq!(v.status, "invalid");
    assert!(v.error.unwrap().message.contains('5'));

    // 声明返回 i32，函数体却留下 i64
    let mut types = TypeSection::new();
    types.ty().function([], [ValType::I32]);
    let mut funcs = FunctionSection::new();
    funcs.function(0);
    let mut code = CodeSection::new();
    let mut f = Function::new([]);
    f.instructions().i64_const(0).end();
    code.function(&f);
    let mut module = Module::new();
    module.section(&types).section(&funcs).section(&code);
    let v = run(&module.finish());
    assert_eq!(v.status, "invalid");
    assert!(v.error.unwrap().message.contains("type mismatch"));
}

#[test]
fn separates_unsupported_proposals_from_corruption() {
    // custom-page-sizes 提案默认未启用
    let mut memories = MemorySection::new();
    memories.memory(MemoryType {
        minimum: 1,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: Some(0),
    });
    let mut module = Module::new();
    module.section(&memories);
    let v = run(&module.finish());
    assert_eq!(v.status, "needs_features");
    assert_eq!(v.required, ["custom-page-sizes"]);
    assert!(v.detected);
    // 截断的模块即使打开全部提案也无法通过
    let v = run(&raw(&[1, 0x80]));
    assert_eq!(v.status, "invalid");
}

#[test]
fn can_be_cancelled() {
    assert!(validate(&crate::fixtures::many(1000), &|| true).is_none());
}
