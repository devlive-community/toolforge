use super::*;
use tf_plugin_api::{LogLevel, ToolPlugin};

/// 测试用的任务上下文：内存文件 + 取消状态
struct Ctx {
    cancelled: bool,
}

impl TaskContext for Ctx {
    fn log(&self, _level: LogLevel, _code: &str, _params: Value) {}
    fn progress(&self, _done: u64, _total: u64) {}
    fn stage(&self, _code: &str) {}
    fn is_cancelled(&self) -> bool {
        self.cancelled
    }
    fn open_file(&self, _path: &str) -> PluginResult<Box<dyn std::io::Read + Send>> {
        Ok(Box::new(std::io::Cursor::new(CBOR_SAMPLE.to_vec())))
    }
    fn file_size(&self, _path: &str) -> PluginResult<u64> {
        Ok(CBOR_SAMPLE.len() as u64)
    }
}

/// CBOR {"a": 1, "b": [2, 3]}
const CBOR_SAMPLE: &[u8] = &[0xa2, 0x61, b'a', 0x01, 0x61, b'b', 0x82, 0x02, 0x03];

fn inspect_args(args: Value) -> Value {
    args
}

#[test]
fn inspect_cbor_from_hex() {
    let plugin = BinaryInspector::default();
    let hex = model::hex(CBOR_SAMPLE);
    let out = plugin
        .run_task(
            "inspect",
            inspect_args(serde_json::json!({"format": "cbor", "encoding": "hex", "text": hex})),
            &Ctx { cancelled: false },
        )
        .unwrap();
    assert_eq!(out["handle"], 1);
    assert_eq!(out["overall"], "passed");
    assert_eq!(out["format"], "cbor");
    assert_eq!(out["root"]["kind"], "map");
    let entries = out["root"]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["key"]["value"], "a");
    // 数组子节点带原始字节区间
    assert_eq!(entries[1]["value"]["children"][0]["offset"], 7);
    assert_eq!(entries[1]["value"]["children"][0]["length"], 1);
}

#[test]
fn inspect_msgpack_from_base64() {
    let plugin = BinaryInspector::default();
    // 0x92, 0x01, 0x02 → [1, 2]
    let out = plugin
        .run_task(
            "inspect",
            inspect_args(
                serde_json::json!({"format": "msgpack", "encoding": "base64", "text": "kgEC"}),
            ),
            &Ctx { cancelled: false },
        )
        .unwrap();
    assert_eq!(out["root"]["kind"], "array");
    assert_eq!(out["root"]["children"][0]["value"], "1");
    let out = plugin
        .run_task(
            "inspect",
            inspect_args(
                serde_json::json!({"format": "msgpack", "encoding": "base64", "text": "kgEC==="}),
            ),
            &Ctx { cancelled: false },
        )
        .unwrap();
    assert_eq!(out["overall"], "passed");
}

#[test]
fn transport_errors_are_separate_from_structure_errors() {
    let plugin = BinaryInspector::default();
    // 非法 hex 字符：运输编码错误，无 root，不冒充解析失败
    let out = plugin
        .run_task(
            "inspect",
            inspect_args(serde_json::json!({"format": "cbor", "encoding": "hex", "text": "zz"})),
            &Ctx { cancelled: false },
        )
        .unwrap();
    assert_eq!(out["overall"], "issues");
    assert_eq!(out["diagnostics"][0]["code"], "transport.hex");
    assert!(out["root"].is_null());
    // base64 非法字符
    let out = plugin
        .run_task(
            "inspect",
            inspect_args(
                serde_json::json!({"format": "cbor", "encoding": "base64", "text": "!!!"}),
            ),
            &Ctx { cancelled: false },
        )
        .unwrap();
    assert_eq!(out["diagnostics"][0]["code"], "transport.base64");
}

#[test]
fn trailing_data_reported_separately() {
    let plugin = BinaryInspector::default();
    let out = plugin
        .run_task(
            "inspect",
            inspect_args(
                serde_json::json!({"format": "cbor", "encoding": "hex", "text": "010203"}),
            ),
            &Ctx { cancelled: false },
        )
        .unwrap();
    assert_eq!(out["overall"], "issues");
    let trailing = out["trailing"].as_object().unwrap();
    assert_eq!(trailing["offset"], 1);
    assert_eq!(trailing["length"], 2);
    // 多值流不会被自动当成数组：root 仍是第一个值
    assert_eq!(out["root"]["kind"], "uint");
}

#[test]
fn structure_errors_never_pass() {
    let plugin = BinaryInspector::default();
    // CBOR 保留附加信息 0x1c
    let out = plugin
        .run_task(
            "inspect",
            inspect_args(serde_json::json!({"format": "cbor", "encoding": "hex", "text": "1c"})),
            &Ctx { cancelled: false },
        )
        .unwrap();
    assert_eq!(out["overall"], "issues");
    assert_eq!(out["diagnostics"][0]["code"], "cbor.reserved");
    assert!(out["root"].is_null());
    // MessagePack 0xc1 保留码
    let out = plugin
        .run_task(
            "inspect",
            inspect_args(serde_json::json!({"format": "msgpack", "encoding": "hex", "text": "c1"})),
            &Ctx { cancelled: false },
        )
        .unwrap();
    assert_eq!(out["diagnostics"][0]["code"], "mp.reserved");
}

#[test]
fn read_from_task_file_and_cancel() {
    let plugin = BinaryInspector::default();
    let out = plugin
        .run_task(
            "inspect",
            inspect_args(serde_json::json!({"format": "cbor", "path": "/chosen/sample.cbor"})),
            &Ctx { cancelled: false },
        )
        .unwrap();
    assert_eq!(out["overall"], "passed");
    let err = plugin
        .run_task(
            "inspect",
            inspect_args(serde_json::json!({"format": "cbor", "path": "/chosen/sample.cbor"})),
            &Ctx { cancelled: true },
        )
        .unwrap_err();
    assert_eq!(err.code, "task.cancelled");
}

#[test]
fn empty_input_and_unknown_format() {
    let plugin = BinaryInspector::default();
    let out = plugin
        .run_task(
            "inspect",
            inspect_args(serde_json::json!({"format": "cbor", "encoding": "hex", "text": ""})),
            &Ctx { cancelled: false },
        )
        .unwrap();
    assert_eq!(out["diagnostics"][0]["code"], "bin.empty");
    let err = plugin
        .run_task(
            "inspect",
            inspect_args(serde_json::json!({"format": "bogus", "encoding": "hex", "text": "00"})),
            &Ctx { cancelled: false },
        )
        .unwrap_err();
    assert_eq!(err.code, "bin.unknown_format");
}

#[test]
fn export_requires_previous_inspect_and_renders() {
    let plugin = BinaryInspector::default();
    let err = plugin
        .call("export", serde_json::json!({"format": "markdown"}))
        .unwrap_err();
    assert_eq!(err.code, "bin.no_result");
    let hex = model::hex(CBOR_SAMPLE);
    plugin
        .run_task(
            "inspect",
            inspect_args(serde_json::json!({"format": "cbor", "encoding": "hex", "text": hex})),
            &Ctx { cancelled: false },
        )
        .unwrap();
    let out = plugin
        .call("export", serde_json::json!({"format": "markdown"}))
        .unwrap();
    let text = out["content"].as_str().unwrap();
    assert!(text.contains("# Binary Serialization Inspection"));
    assert!(text.contains("- map @0x0+9"));
    let out = plugin
        .call("export", serde_json::json!({"format": "json"}))
        .unwrap();
    let parsed: Value = serde_json::from_str(out["content"].as_str().unwrap()).unwrap();
    assert_eq!(parsed["root"]["kind"], "map");
}

#[test]
fn unknown_function_is_rejected() {
    let err = BinaryInspector::default()
        .call("send", Value::Null)
        .unwrap_err();
    assert_eq!(err.code, "plugin.function_not_found");
}
