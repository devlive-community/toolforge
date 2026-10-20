//! 按平台采集系统代理配置；每个来源独立成功/失败，不互相掩盖。

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

/// 各平台的系统来源列表（不包含进程环境变量）
pub fn collect_sources() -> Vec<super::model::SourceResult> {
    #[cfg(target_os = "macos")]
    {
        vec![macos::system_configuration()]
    }
    #[cfg(windows)]
    {
        vec![windows::wininet_current_user(), windows::winhttp_default()]
    }
    #[cfg(target_os = "linux")]
    {
        vec![linux::gnome_settings()]
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Vec::new()
    }
}
