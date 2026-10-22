//! SystemConfiguration 字典读取的类型守卫回归测试。
//! SCDynamicStore 本身在测试进程会抛 ObjC 异常（无 bundle 上下文），
//! 但字典读取辅助函数可以用合成字典直接验证。

use super::*;
use core_foundation::base::TCFType;

fn test_dict() -> CFDictionary<CFString, CFType> {
    CFDictionary::from_CFType_pairs(&[
        (key("HTTPEnable"), CFNumber::from(1).into_CFType()),
        (key("HTTPProxy"), CFString::new("127.0.0.1").into_CFType()),
        (key("HTTPPort"), CFNumber::from(7890).into_CFType()),
    ])
}

/// SystemConfiguration 的端口值是 CFNumber；按 CFString 包装并读取是
/// 未定义行为，曾让整个应用在打开插件时崩溃。此测试锁死该回归。
#[test]
fn reads_enabled_entry_with_number_port() {
    let dict = test_dict();
    let value = entry(&dict, "HTTPEnable", "HTTPProxy", "HTTPPort", "http")
        .expect("enabled entry must exist");
    assert_eq!(value.host, "127.0.0.1");
    assert_eq!(value.port, 7890);
    assert!(value.warnings.is_empty());
}

#[test]
fn missing_enable_flag_yields_no_entry() {
    let dict = test_dict();
    assert!(entry(&dict, "HTTPSEnable", "HTTPSProxy", "HTTPSPort", "https").is_none());
}

#[test]
fn disabled_flag_yields_no_entry() {
    let dict = CFDictionary::from_CFType_pairs(&[
        (key("HTTPEnable"), CFNumber::from(0).into_CFType()),
        (key("HTTPProxy"), CFString::new("proxy.corp").into_CFType()),
        (key("HTTPPort"), CFNumber::from(8080).into_CFType()),
    ]);
    assert!(entry(&dict, "HTTPEnable", "HTTPProxy", "HTTPPort", "http").is_none());
}

#[test]
fn reads_number_or_boolean_switches() {
    // 开关在 SC 字典里通常是 CFNumber，个别场景可能是 CFBoolean
    let dict = CFDictionary::from_CFType_pairs(&[
        (
            key("ProxyAutoDiscoveryEnable"),
            CFNumber::from(1).into_CFType(),
        ),
        (
            key("ProxyAutoConfigEnable"),
            core_foundation::boolean::CFBoolean::from(true).into_CFType(),
        ),
    ]);
    assert_eq!(bool_value(&dict, "ProxyAutoDiscoveryEnable"), Some(true));
    assert_eq!(bool_value(&dict, "ProxyAutoConfigEnable"), Some(true));
    assert_eq!(bool_value(&dict, "Missing"), None);
}

#[test]
fn skips_wrong_typed_values_without_touching_them() {
    // 值类型与预期不符时必须跳过，绝不能按错误类型包装读取
    let dict = CFDictionary::from_CFType_pairs(&[
        (key("HTTPEnable"), CFNumber::from(1).into_CFType()),
        (key("HTTPProxy"), CFNumber::from(42).into_CFType()),
        (key("HTTPPort"), CFString::new("not-a-number").into_CFType()),
    ]);
    let value = entry(&dict, "HTTPEnable", "HTTPProxy", "HTTPPort", "http").expect("entry");
    assert_eq!(value.host, "");
    assert_eq!(value.port, 0);
    assert_eq!(
        value.warnings,
        vec!["proxy.empty_host".to_owned(), "proxy.bad_port".to_owned()]
    );
}
