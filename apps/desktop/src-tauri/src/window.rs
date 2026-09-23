use serde_json::{Value, json};
use tauri::webview::PageLoadEvent;
use tauri::window::Color;
use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder};

/// 创建主窗口。
///
/// 偏好设置来自 SQLite，通过 initialization_script 在页面脚本执行前注入，
/// 用于首帧设置主题（避免闪烁）；窗口先隐藏，页面加载完成后由 Rust 显示，
/// 不依赖前端脚本是否执行成功。
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

    // 与 tokens.css 中 --tf-bg 保持一致，避免窗口显示瞬间闪白
    let background = match prefs.get("theme").and_then(Value::as_str) {
        Some("dark") => Color(0x0f, 0x13, 0x11, 0xff),
        _ => Color(0xf7, 0xf9, 0xf8, 0xff),
    };

    let builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
        .title("ToolForge")
        .inner_size(1440.0, 900.0)
        .min_inner_size(1100.0, 700.0)
        .center()
        .visible(false)
        .background_color(background)
        .initialization_script(script)
        .on_page_load(|window, payload| {
            if payload.event() == PageLoadEvent::Finished {
                let _ = window.show();
            }
        });

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
