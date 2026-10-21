use crate::env;
use std::process::Command;

#[test]
fn collects_case_variants_and_masks_values() {
    const CHILD_MARKER: &str = "TOOLFORGE_PROXY_INSPECTOR_ENV_TEST_CHILD";
    if std::env::var_os(CHILD_MARKER).is_none() {
        // Give the child its fixture at launch. Never mutate the parallel
        // test runner's process environment, even while holding a Rust mutex.
        let mut child = Command::new(std::env::current_exe().unwrap());
        child.args([
            "--exact",
            "env::tests::collects_case_variants_and_masks_values",
            "--nocapture",
        ]);
        for (name, _) in env::PROXY_VARS {
            child.env_remove(name);
        }
        let status = child
            .env(CHILD_MARKER, "1")
            .env("HTTP_PROXY", "http://user:secret@proxy.corp:8080")
            .env("HTTPS_PROXY", "https://proxy.corp:443")
            .env("NO_PROXY", "localhost,.corp")
            .env("https_proxy", "http://other:3128")
            .env("ALL_PROXY", "   ")
            .status()
            .expect("environment fixture child should run");
        assert!(status.success(), "environment fixture child failed");
        return;
    }
    let vars = env::collect();
    let find = |name: &str| vars.iter().find(|v| v.name == name).cloned();
    let http = find("HTTP_PROXY").expect("HTTP_PROXY should exist");
    // 凭据已遮罩
    assert_eq!(http.value, "http://user:••••@proxy.corp:8080");
    assert!(!http.is_exclusions);
    // Unix preserves distinct names; Windows resolves both case variants
    // to the value from the last assignment to its case-insensitive key.
    let lower = find("https_proxy").expect("https_proxy should exist");
    assert_eq!(lower.value, "http://other:3128");
    let upper = find("HTTPS_PROXY").expect("HTTPS_PROXY should exist");
    #[cfg(not(windows))]
    assert_eq!(upper.value, "https://proxy.corp:443");
    #[cfg(windows)]
    assert_eq!(upper.value, "http://other:3128");
    // NO_PROXY 是排除列表：不遮罩，标记为排除
    let no_proxy = find("NO_PROXY").expect("NO_PROXY should exist");
    assert_eq!(no_proxy.value, "localhost,.corp");
    assert!(no_proxy.is_exclusions);
    // 空值跳过
    assert!(find("ALL_PROXY").is_none());
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
