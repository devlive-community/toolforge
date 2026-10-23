use std::io::Cursor;
use std::time::Duration;

use tf_core::store::PLUGIN_SCOPE;
use tf_core::{AgentPath, InterfaceGrant};

use super::*;
use crate::builtin;

const HASH: &str = "org.devlive.toolforge.hash";

/// 收集服务输出的缓冲区
#[derive(Clone, Default)]
struct Output(Arc<Mutex<Vec<u8>>>);

impl Write for Output {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        lock(&self.0).extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Output {
    fn messages(&self) -> Vec<Value> {
        String::from_utf8_lossy(&lock(&self.0))
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    /// 等待某条消息出现（tools/call 在线程中回复）
    fn wait(&self, matches: impl Fn(&Value) -> bool) -> Value {
        for _ in 0..500 {
            if let Some(found) = self.messages().into_iter().find(&matches) {
                return found;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("message not found in {:?}", self.messages());
    }

    fn reply(&self, id: i64) -> Value {
        self.wait(|m| m["id"] == id && m.get("method").is_none())
    }
}

fn store(enabled: bool, paths: &[AgentPath]) -> Store {
    let store = Store::open_in_memory().unwrap();
    store
        .kv_set(tf_host::agent::ENABLED_KEY, &json!(enabled))
        .unwrap();
    store
        .interface_grant_set(&InterfaceGrant {
            channel: "agent".into(),
            plugin_id: HASH.into(),
            function: PLUGIN_SCOPE.into(),
            mode: "allow".into(),
            uses: None,
        })
        .unwrap();
    store
        .interface_grant_set(&InterfaceGrant {
            channel: "agent".into(),
            plugin_id: HASH.into(),
            function: "manifest_create".into(),
            mode: "off".into(),
            uses: None,
        })
        .unwrap();
    for path in paths {
        store.agent_path_set(path).unwrap();
    }
    store
}

fn start(store: Store) -> (Arc<Server>, Output) {
    let output = Output::default();
    let server = server(
        Catalog::new(builtin::all(), None),
        Some(store),
        "en-US",
        Box::new(output.clone()),
    );
    (server, output)
}

fn send(server: &Arc<Server>, messages: &[Value]) {
    let text: String = messages.iter().map(|m| format!("{m}\n")).collect();
    run(server, Cursor::new(text));
}

fn initialize(elicitation: bool) -> Value {
    let capabilities = if elicitation {
        json!({ "elicitation": {} })
    } else {
        json!({})
    };
    json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2025-06-18", "capabilities": capabilities,
                        "clientInfo": { "name": "test", "version": "1" } } })
}

fn call(id: i64, name: &str, arguments: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": name, "arguments": arguments } })
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tfp-mcp-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

#[test]
fn initialize_negotiates_the_protocol_version() {
    let (server, output) = start(store(true, &[]));
    send(&server, &[initialize(false)]);
    let reply = output.reply(1);
    assert_eq!(reply["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(reply["result"]["serverInfo"]["name"], "toolforge");

    let (server, output) = start(store(true, &[]));
    let mut old = initialize(false);
    old["params"]["protocolVersion"] = json!("1999-01-01");
    send(&server, &[old]);
    assert_eq!(
        output.reply(1)["result"]["protocolVersion"],
        PROTOCOL_VERSIONS[0]
    );
}

#[test]
fn tools_list_shows_only_granted_functions() {
    let (server, output) = start(store(false, &[]));
    send(
        &server,
        &[json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })],
    );
    assert_eq!(
        output.reply(2)["result"]["tools"],
        json!([]),
        "agents are off"
    );

    let (server, output) = start(store(true, &[]));
    send(
        &server,
        &[json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })],
    );
    let tools = output.reply(2)["result"]["tools"]
        .as_array()
        .unwrap()
        .clone();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"hash__hash_text"));
    assert!(names.contains(&"hash__hash_files"), "asks but stays listed");
    assert!(!names.contains(&"hash__manifest_create"), "turned off");
    let text = tools
        .iter()
        .find(|t| t["name"] == "hash__hash_text")
        .unwrap();
    assert_eq!(text["annotations"]["readOnlyHint"], true);
    assert!(
        text["inputSchema"]["properties"]["input"]
            .get("x-stdin")
            .is_none()
    );
}

#[test]
fn calls_allowed_functions_and_records_history() {
    let (server, output) = start(store(true, &[]));
    send(
        &server,
        &[call(
            3,
            "hash__hash_text",
            json!({ "input": "hello", "algorithms": ["md5"] }),
        )],
    );
    let result = &output.reply(3)["result"];
    assert_eq!(result["isError"], false, "{result}");
    assert_eq!(
        result["structuredContent"]["results"][0]["digest"],
        "5d41402abc4b2a76b9719d911017c592"
    );
    let history = server
        .store
        .as_ref()
        .unwrap()
        .history_list("", 10, 0)
        .unwrap();
    assert_eq!(history[0].source, "agent");
    assert_eq!(history[0].input, "hello");
}

#[test]
fn file_access_needs_granted_folders_and_confirmation() {
    let granted = temp_dir("granted");
    let outside = temp_dir("outside");
    std::fs::write(granted.join("a.txt"), "a").unwrap();
    std::fs::write(outside.join("b.txt"), "b").unwrap();
    let paths = [AgentPath {
        path: granted.to_string_lossy().into(),
        access: "read".into(),
    }];

    let (server, output) = start(store(true, &paths));
    send(
        &server,
        &[
            initialize(false),
            call(
                4,
                "hash__hash_files",
                json!({ "paths": [outside.join("b.txt")] }),
            ),
            call(
                5,
                "hash__hash_files",
                json!({ "paths": [granted.join("a.txt")] }),
            ),
            call(6, "hash__hash_files", json!({ "paths": ["relative.txt"] })),
        ],
    );
    let text = |id| {
        output.reply(id)["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert!(text(4).ends_with("[agent.path_denied]"), "{}", text(4));
    assert!(
        text(5).ends_with("[agent.needs_confirmation]"),
        "native file access asks: {}",
        text(5)
    );
    assert!(text(6).ends_with("[agent.relative_path]"), "{}", text(6));
    std::fs::remove_dir_all(granted).unwrap();
    std::fs::remove_dir_all(outside).unwrap();
}

#[test]
fn asks_the_user_through_elicitation() {
    let granted = temp_dir("elicit");
    std::fs::write(granted.join("a.txt"), "hello").unwrap();
    let paths = [AgentPath {
        path: granted.to_string_lossy().into(),
        access: "read".into(),
    }];
    let (server, output) = start(store(true, &paths));
    send(
        &server,
        &[
            initialize(true),
            call(
                7,
                "hash__hash_files",
                json!({ "paths": [granted.join("a.txt")] }),
            ),
        ],
    );

    let request = output.wait(|m| m["method"] == "elicitation/create");
    assert!(
        request["params"]["message"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("hash")
    );
    send(
        &server,
        &[json!({ "jsonrpc": "2.0", "id": request["id"], "result": { "action": "accept" } })],
    );
    let result = &output.reply(7)["result"];
    assert_eq!(result["isError"], false, "{result}");

    send(
        &server,
        &[call(
            8,
            "hash__hash_files",
            json!({ "paths": [granted.join("a.txt")] }),
        )],
    );
    let second = output.wait(|m| m["method"] == "elicitation/create" && m["id"] != request["id"]);
    send(
        &server,
        &[json!({ "jsonrpc": "2.0", "id": second["id"], "result": { "action": "decline" } })],
    );
    let declined = output.reply(8)["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(declined.ends_with("[agent.declined]"), "{declined}");
    std::fs::remove_dir_all(granted).unwrap();
}

#[test]
fn protocol_errors() {
    let (server, output) = start(store(true, &[]));
    send(
        &server,
        &[
            call(9, "hash__nothing", json!({})),
            json!({ "jsonrpc": "2.0", "id": 10, "method": "resources/list" }),
            json!({ "jsonrpc": "2.0", "id": 11, "method": "ping" }),
        ],
    );
    run(&server, Cursor::new("not json\n"));
    assert_eq!(output.reply(9)["error"]["code"], INVALID_PARAMS);
    assert_eq!(output.reply(10)["error"]["code"], METHOD_NOT_FOUND);
    assert_eq!(output.reply(11)["result"], json!({}));
    assert!(
        output
            .messages()
            .iter()
            .any(|m| m["error"]["code"] == PARSE_ERROR)
    );
}
