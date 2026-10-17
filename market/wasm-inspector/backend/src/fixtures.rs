//! 测试用模块：用 wasm-encoder 构造，覆盖导入、定义、导出、name / producers / 自定义 section。

use wasm_encoder::{
    CodeSection, ConstExpr, CustomSection, EntityType, ExportKind, ExportSection, Function,
    FunctionSection, GlobalSection, GlobalType, ImportSection, MemoryType, Module, NameMap,
    NameSection, ProducersField, ProducersSection, RefType, TableSection, TableType, TypeSection,
    ValType,
};

/// 1 个导入函数 + 2 个定义函数（add、run）、导入内存、表、全局、导出、三个 custom section
pub fn sample() -> Vec<u8> {
    let mut types = TypeSection::new();
    types
        .ty()
        .function([ValType::I32, ValType::I32], [ValType::I32]);
    types.ty().function([], []);
    let mut imports = ImportSection::new();
    imports.import("env", "log", EntityType::Function(1));
    imports.import(
        "env",
        "mem",
        EntityType::Memory(MemoryType {
            minimum: 1,
            maximum: Some(2),
            memory64: false,
            shared: false,
            page_size_log2: None,
        }),
    );
    let mut funcs = FunctionSection::new();
    funcs.function(0);
    funcs.function(1);
    let mut tables = TableSection::new();
    tables.table(TableType {
        element_type: RefType::FUNCREF,
        minimum: 1,
        maximum: None,
        table64: false,
        shared: false,
    });
    let mut globals = GlobalSection::new();
    globals.global(
        GlobalType {
            val_type: ValType::I32,
            mutable: true,
            shared: false,
        },
        &ConstExpr::i32_const(0),
    );
    let mut exports = ExportSection::new();
    exports.export("add", ExportKind::Func, 1);
    exports.export("table", ExportKind::Table, 0);
    exports.export("counter", ExportKind::Global, 0);
    let mut code = CodeSection::new();
    let mut add = Function::new([(2, ValType::I64)]);
    add.instructions().local_get(0).local_get(1).i32_add().end();
    code.function(&add);
    let mut run = Function::new([]);
    run.instructions().call(0).end();
    code.function(&run);
    let mut names = NameSection::new();
    let mut map = NameMap::new();
    map.append(1, "add");
    map.append(2, "run");
    names.functions(&map);
    let mut producers = ProducersSection::new();
    let mut field = ProducersField::new();
    field.value("rustc", "1.99.0");
    producers.field("processed-by", &field);

    let mut module = Module::new();
    module
        .section(&types)
        .section(&imports)
        .section(&funcs)
        .section(&tables)
        .section(&globals)
        .section(&exports)
        .section(&code)
        .section(&names)
        .section(&producers)
        .section(&CustomSection {
            name: "note".into(),
            data: vec![7u8; 100].into(),
        });
    module.finish()
}

/// 文件头后接一段原始 section 字节
pub fn raw(sections: &[u8]) -> Vec<u8> {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    bytes.extend_from_slice(sections);
    bytes
}

/// `count` 个相同的小函数，用于测试大模块的取消与预算
pub fn many(count: u32) -> Vec<u8> {
    let mut types = TypeSection::new();
    types.ty().function([ValType::I32], [ValType::I32]);
    let mut funcs = FunctionSection::new();
    let mut code = CodeSection::new();
    for _ in 0..count {
        funcs.function(0);
        let mut f = Function::new([]);
        f.instructions()
            .local_get(0)
            .i32_const(1)
            .i32_add()
            .i32_const(2)
            .i32_mul()
            .end();
        code.function(&f);
    }
    let mut module = Module::new();
    module.section(&types).section(&funcs).section(&code);
    module.finish()
}
