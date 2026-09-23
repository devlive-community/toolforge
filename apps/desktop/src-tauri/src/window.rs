use serde_json::{Value, json};
use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder};

/// 创建主窗口。
///
/// 偏好设置来自 SQLite，通过 initialization_script 在页面脚本执行前注入，
/// 用于首帧设置主题（避免闪烁）；窗口先隐藏，前端首次渲染完成后再显示。
pub fn create_main(app: &AppHandle, prefs: &Value) -> tauri::Result<()> {
    let boot = json!({ "prefs": prefs, "os": std::env::consts::OS });
    let script = format!(
        r#"window.__TF_BOOT__ = {boot};
(function () {{
  var t = (window.__TF_BOOT__.prefs && window.__TF_BOOT__.prefs.theme) || 'system';
  var dark = t === 'dark' || (t === 'system' && matchMedia('(prefers-color-scheme: dark)').matches);
  document.documentElement.classList.toggle('dark', dark);
}})();"#
    );

    let builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
        .title("ToolForge")
        .inner_size(1440.0, 900.0)
        .min_inner_size(1100.0, 700.0)
        .center()
        .visible(false)
        .initialization_script(script);

    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(tauri::LogicalPosition::new(18.0, 26.0));

    #[cfg(not(target_os = "macos"))]
    let builder = builder.decorations(false);

    builder.build()?;
    Ok(())
}
