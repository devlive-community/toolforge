use crate::env;
use std::sync::Mutex;

/// env 操作不是线程安全的：串行执行避免测试并行污染
static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn collects_case_variants_and_masks_values() {
    let _guard = ENV_LOCK.lock().unwrap();
    unsafe {
        std::env::set_var("HTTP_PROXY", "http://user:secret@proxy.corp:8080");
        std::env::set_var("HTTPS_PROXY", "https://proxy.corp:443");
        std::env::set_var("NO_PROXY", "localhost,.corp");
        std::env::set_var("https_proxy", "http://other:3128");
        std::env::set_var("ALL_PROXY", "   ");
    }
    let vars = env::collect();
    let find = |name: &str| vars.iter().find(|v| v.name == name).cloned();
    let http = find("HTTP_PROXY").expect("HTTP_PROXY should exist");
    // 凭据已遮罩
    assert_eq!(http.value, "http://user:••••@proxy.corp:8080");
    assert!(!http.is_exclusions);
    // 大小写并存：各值保留
    let lower = find("https_proxy").expect("https_proxy should exist");
    assert_eq!(lower.value, "http://other:3128");
    let upper = find("HTTPS_PROXY").expect("HTTPS_PROXY should exist");
    assert_eq!(upper.value, "https://proxy.corp:443");
    // NO_PROXY 是排除列表：不遮罩，标记为排除
    let no_proxy = find("NO_PROXY").expect("NO_PROXY should exist");
    assert_eq!(no_proxy.value, "localhost,.corp");
    assert!(no_proxy.is_exclusions);
    // 空值跳过
    assert!(find("ALL_PROXY").is_none());
    unsafe {
        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("HTTPS_PROXY");
        std::env::remove_var("NO_PROXY");
        std::env::remove_var("https_proxy");
        std::env::remove_var("ALL_PROXY");
    }
}

#[test]
fn whitelist_names_are_complete() {
    // 四个功能 + 大小写 = 8 个槽位
    assert_eq!(env::PROXY_VARS.len(), 8);
    assert!(env::PROXY_VARS.iter().all(|(name, _)| {
        let lower = name.to_ascii_lowercase();
        matches!(
            lower.as_str(),
            "http_proxy" | "https_proxy" | "all_proxy" | "no_proxy"
        )
    }));
}
