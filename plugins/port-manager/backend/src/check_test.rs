use std::io::Read;
use std::net::TcpListener;
use std::path::PathBuf;

use serde_json::Value;
use tf_plugin_api::LogLevel;

use super::*;

/// 依赖端口开闭状态的测试需要串行，避免刚释放的端口被其他测试占用
static PORTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct Ctx;

impl TaskContext for Ctx {
    fn log(&self, _: LogLevel, _: &str, _: Value) {}
    fn progress(&self, _: u64, _: u64) {}
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        false
    }
    fn open_file(&self, _: &str) -> PluginResult<Box<dyn Read + Send>> {
        Err(PluginError::new("fs.not_found"))
    }
    fn file_size(&self, _: &str) -> PluginResult<u64> {
        Err(PluginError::new("fs.not_found"))
    }
    fn resource_path(&self, id: &str) -> PluginResult<PathBuf> {
        Err(PluginError::new("resource.missing").with("id", id))
    }
}

#[test]
fn parses_port_lists() {
    assert_eq!(
        parse_ports("443, 80\n8000-8002 80").unwrap(),
        vec![80, 443, 8000, 8001, 8002]
    );
    assert_eq!(parse_ports("0").unwrap_err().code, "port.invalid_ports");
    assert_eq!(parse_ports("10-5").unwrap_err().code, "port.invalid_ports");
    assert_eq!(parse_ports("http").unwrap_err().code, "port.invalid_ports");
    assert_eq!(parse_ports("70000").unwrap_err().code, "port.invalid_ports");
    assert_eq!(
        parse_ports("1-2000").unwrap_err().code,
        "port.too_many_ports"
    );
    assert_eq!(parse_ports(" , ").unwrap_err().code, "port.no_ports");
}

#[test]
fn checks_local_ports() {
    let _guard = PORTS.lock().unwrap_or_else(|e| e.into_inner());
    let open = TcpListener::bind("127.0.0.1:0").unwrap();
    let open_port = open.local_addr().unwrap().port();
    // 绑定后立即释放得到一个几乎确定关闭的端口
    let closed_port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let args = Args {
        host: "127.0.0.1".into(),
        ports: format!("{open_port},{closed_port}"),
        timeout_ms: 500,
    };
    let report = run(args, &Ctx).unwrap();
    assert_eq!(report.address, "127.0.0.1");
    assert_eq!(report.open, 1);
    let find = |port| report.results.iter().find(|r| r.port == port).unwrap();
    assert!(find(open_port).open && find(open_port).ms.is_some());
    // 并行测试时端口状态可能短暂变化，只要求判定为未开放
    let closed = find(closed_port);
    assert!(!closed.open, "{closed:?}");
    assert!(
        matches!(closed.reason.as_deref(), Some("refused" | "other")),
        "{closed:?}"
    );
}

#[test]
fn reports_bad_hosts() {
    let args = |host: &str| Args {
        host: host.into(),
        ports: "80".into(),
        timeout_ms: 500,
    };
    assert_eq!(run(args(""), &Ctx).unwrap_err().code, "port.no_host");
    // 开启 fake-ip 的代理会为任何域名返回 198.18.x.x，此时应标记出来
    match run(args("no-such-host.invalid"), &Ctx) {
        Err(err) => assert_eq!(err.code, "port.resolve_failed"),
        Ok(report) => assert!(report.fake_ip, "{report:?}"),
    }
    assert!(is_fake_ip(&"198.19.0.1:0".parse().unwrap()));
    assert!(!is_fake_ip(&"198.20.0.1:0".parse().unwrap()));
}

#[test]
fn tries_every_resolved_address() {
    let _guard = PORTS.lock().unwrap_or_else(|e| e.into_inner());
    // 只监听 IPv4，而 localhost 可能先解析为 ::1
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let args = Args {
        host: "localhost".into(),
        ports: port.to_string(),
        timeout_ms: 500,
    };
    let report = run(args, &Ctx).unwrap();
    assert!(report.results[0].open, "{report:?}");
    assert!(report.address.contains("127.0.0.1"), "{}", report.address);
}
