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

    // macOS 保留原生窗口（圆角、阴影、边缘缩放），只隐藏系统的红黄绿按钮；
    // 其他平台使用无边框窗口。三个平台统一使用应用自绘的窗口控制按钮。
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);

    #[cfg(not(target_os = "macos"))]
    let builder = builder.decorations(false);

    let window = builder.build()?;

    // 关闭窗口前先确认（自绘关闭按钮、⌘W 都会走到这里）
    let app_handle = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event
            && !crate::lifecycle::request_quit(&app_handle)
        {
            api.prevent_close();
        }
    });

    #[cfg(target_os = "macos")]
    {
        macos::hide_window_buttons(&window);
        let handle = window.clone();
        // 全屏切换、获得焦点等时机系统可能重新显示按钮，需要再次隐藏
        window.on_window_event(move |event| {
            if matches!(
                event,
                tauri::WindowEvent::Resized(_)
                    | tauri::WindowEvent::Focused(_)
                    | tauri::WindowEvent::ThemeChanged(_)
            ) {
                macos::hide_window_buttons(&handle);
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = window;

    Ok(())
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2_app_kit::{NSWindow, NSWindowButton};
    use tauri::WebviewWindow;

    /// 隐藏 NSWindow 的关闭 / 最小化 / 缩放按钮
    pub fn hide_window_buttons(window: &WebviewWindow) {
        let Ok(ptr) = window.ns_window() else {
            return;
        };
        // SAFETY: ns_window 返回当前窗口有效的 NSWindow 指针；窗口事件与 setup 均在主线程执行
        let ns_window = unsafe { &*(ptr as *const NSWindow) };
        for kind in [
            NSWindowButton::CloseButton,
            NSWindowButton::MiniaturizeButton,
            NSWindowButton::ZoomButton,
        ] {
            if let Some(button) = ns_window.standardWindowButton(kind) {
                button.setHidden(true);
            }
        }
    }
}
