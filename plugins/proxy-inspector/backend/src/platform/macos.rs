//! macOS：SystemConfiguration 代理字典（SCDynamicStoreCopyProxies，只读快照）。

use crate::mask::{mask_host, mask_url};
use crate::model::{Entry, Source, check_entry};
use core_foundation::array::CFArray;
use core_foundation::base::{CFGetTypeID, CFType, TCFType, ToVoid};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use system_configuration::dynamic_store::SCDynamicStoreBuilder;

fn key(text: &str) -> CFString {
    CFString::new(text)
}

fn type_id_of(value: &CFType) -> usize {
    unsafe { CFGetTypeID(value.to_void()) }
}

fn string_value(dict: &CFDictionary<CFString, CFType>, name: &str) -> Option<String> {
    let value = dict.find(key(name))?;
    // 只有确为 CFString 才能按 CFString 读取；对其他类型调用 CFString API
    // 会抛 ObjC 异常直接崩掉宿主进程（曾因此让打开插件时整个应用退出）
    if type_id_of(&value) != CFString::type_id() {
        return None;
    }
    // 字典值的生存期绑定字典本身，快照读取立即转 String
    let text = unsafe { CFString::wrap_under_get_rule(value.to_void() as *const _) };
    Some(text.to_string())
}

/// 读取数值（开关/端口）；开关可能是 CFNumber 或 CFBoolean
fn number_value(dict: &CFDictionary<CFString, CFType>, name: &str) -> Option<i64> {
    let value = dict.find(key(name))?;
    let type_id = type_id_of(&value);
    if type_id == CFBoolean::type_id() {
        let flag = unsafe { CFBoolean::wrap_under_get_rule(value.to_void() as *const _) };
        return Some(i64::from(flag == CFBoolean::true_value()));
    }
    if type_id != CFNumber::type_id() {
        return None;
    }
    let number = unsafe { CFNumber::wrap_under_get_rule(value.to_void() as *const _) };
    number.to_i64()
}

fn bool_value(dict: &CFDictionary<CFString, CFType>, name: &str) -> Option<bool> {
    number_value(dict, name).map(|value| value != 0)
}

fn entry(
    dict: &CFDictionary<CFString, CFType>,
    enable_key: &str,
    host_key: &str,
    port_key: &str,
    protocol: &'static str,
) -> Option<Entry> {
    if bool_value(dict, enable_key) != Some(true) {
        return None;
    }
    let host = string_value(dict, host_key).unwrap_or_default();
    let port = number_value(dict, port_key)
        .map(|value| value.clamp(0, u32::MAX as i64) as u32)
        .or_else(|| string_value(dict, port_key).and_then(|text| text.trim().parse::<u32>().ok()))
        .unwrap_or(0);
    let warnings = check_entry(&host, port);
    Some(Entry {
        protocol: protocol.to_owned(),
        host: mask_host(&host),
        port,
        warnings,
    })
}

fn string_array(dict: &CFDictionary<CFString, CFType>, name: &str) -> Option<Vec<String>> {
    let value = dict.find(key(name))?;
    if type_id_of(&value) != CFArray::<CFType>::type_id() {
        return None;
    }
    let array = unsafe { CFArray::<CFType>::wrap_under_get_rule(value.to_void() as *const _) };
    Some(
        array
            .iter()
            .filter_map(|item| {
                if unsafe { CFGetTypeID(item.to_void()) } == CFString::type_id() {
                    let text = unsafe { CFString::wrap_under_get_rule(item.to_void() as *const _) };
                    Some(text.to_string())
                } else {
                    None
                }
            })
            .collect(),
    )
}

/// 读取 SystemConfiguration 全局代理字典；读取失败整体报错，不吞成直连
pub fn system_configuration() -> Result<Source, crate::model::SourceFailure> {
    let fail = |code: &str| crate::model::SourceFailure {
        id: "macos-sc".to_owned(),
        code: code.to_owned(),
    };
    let store = SCDynamicStoreBuilder::new("org.devlive.toolforge.proxy-inspector").build();
    let Some(dict) = store.get_proxies() else {
        return Err(fail("proxy.sc_unavailable"));
    };
    let entries = [
        entry(&dict, "HTTPEnable", "HTTPProxy", "HTTPPort", "http"),
        entry(&dict, "HTTPSEnable", "HTTPSProxy", "HTTPSPort", "https"),
        entry(&dict, "SOCKSEnable", "SOCKSProxy", "SOCKSPort", "socks"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let pac_url = bool_value(&dict, "ProxyAutoConfigEnable")
        .filter(|enabled| *enabled)
        .and_then(|_| string_value(&dict, "ProxyAutoConfigURLString"))
        .map(|url| mask_url(&url));
    let auto_discover = bool_value(&dict, "ProxyAutoDiscoveryEnable");
    let exclusions = string_array(&dict, "ExceptionsList").unwrap_or_default();
    let raw = crate::mask::mask_raw(&dict_to_json(&dict));
    Ok(crate::model::build_source(
        "macos-sc",
        "system",
        entries,
        auto_discover,
        pac_url,
        exclusions,
        Some(raw),
    ))
}

/// 字典 → JSON（字符串/数字/布尔/字符串数组），供“原始配置”展开核对
fn dict_to_json(dict: &CFDictionary<CFString, CFType>) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    let (keys, values) = dict.get_keys_and_values();
    for (key, value) in keys.into_iter().zip(values) {
        let name = unsafe { CFString::wrap_under_get_rule(key as *const _) }.to_string();
        let type_id = unsafe { CFGetTypeID(value) };
        let json = if type_id == CFString::type_id() {
            serde_json::Value::String(
                unsafe { CFString::wrap_under_get_rule(value as *const _) }.to_string(),
            )
        } else if type_id == CFNumber::type_id() {
            let number = unsafe { CFNumber::wrap_under_get_rule(value as *const _) };
            number
                .to_i64()
                .map(|n| serde_json::Value::Number(n.into()))
                .unwrap_or(serde_json::Value::Null)
        } else if type_id == CFBoolean::type_id() {
            let flag = unsafe { CFBoolean::wrap_under_get_rule(value as *const _) };
            serde_json::Value::Bool(flag == CFBoolean::true_value())
        } else if type_id == CFArray::<CFType>::type_id() {
            let array = unsafe { CFArray::<CFType>::wrap_under_get_rule(value as *const _) };
            serde_json::Value::Array(
                array
                    .iter()
                    .filter_map(|item| {
                        if unsafe { CFGetTypeID(item.to_void()) } == CFString::type_id() {
                            let text = unsafe {
                                CFString::wrap_under_get_rule(item.to_void() as *const _)
                            };
                            Some(serde_json::Value::String(text.to_string()))
                        } else {
                            None
                        }
                    })
                    .collect(),
            )
        } else {
            continue;
        };
        map.insert(name, json);
    }
    serde_json::Value::Object(map)
}

#[cfg(test)]
#[path = "macos_test.rs"]
mod tests;
