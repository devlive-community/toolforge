//! 退出确认：关闭窗口、⌘Q、菜单退出都先交给前端弹出确认框。
//!
//! 为避免前端异常时应用无法退出，前端收到请求后需要调用 `app_close_ack` 应答；
//! 超时未应答则直接退出。

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;
use crate::commands::PREFS_KEY;

/// 通知前端弹出退出确认框
pub const CLOSE_REQUESTED: &str = "app://close-requested";
const ACK_TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Default)]
pub struct QuitGuard {
    /// 用户已确认退出（或正在重启），之后的关闭 / 退出请求直接放行
    confirmed: AtomicBool,
    requested: AtomicU64,
    acked: AtomicU64,
}

impl QuitGuard {
    pub fn confirm(&self) {
        self.confirmed.store(true, Ordering::SeqCst);
    }
}

/// 偏好设置中关闭了退出确认时直接退出
fn confirm_enabled(app: &AppHandle) -> bool {
    app.try_state::<AppState>()
        .and_then(|state| state.store.kv_get(PREFS_KEY).ok().flatten())
        .and_then(|prefs| prefs.get("confirmQuit").and_then(|v| v.as_bool()))
        .unwrap_or(true)
}

/// 处理一次关闭 / 退出请求；返回 true 表示可以立即退出
pub fn request_quit(app: &AppHandle) -> bool {
    let guard = app.state::<QuitGuard>();
    if guard.confirmed.load(Ordering::SeqCst) || !confirm_enabled(app) {
        guard.confirm();
        return true;
    }
    let seq = guard.requested.fetch_add(1, Ordering::SeqCst) + 1;
    crate::window::show_main(app);
    let _ = app.emit(CLOSE_REQUESTED, seq);

    let handle = app.clone();
    thread::spawn(move || {
        thread::sleep(ACK_TIMEOUT);
        let guard = handle.state::<QuitGuard>();
        if guard.acked.load(Ordering::SeqCst) < seq {
            guard.confirm();
            handle.exit(0);
        }
    });
    false
}

/// 前端已收到退出请求并显示确认框
#[tauri::command]
pub fn app_close_ack(guard: tauri::State<'_, QuitGuard>, seq: u64) {
    guard.acked.fetch_max(seq, Ordering::SeqCst);
}

/// 用户确认退出
#[tauri::command]
pub fn app_quit(app: AppHandle, guard: tauri::State<'_, QuitGuard>) {
    guard.confirm();
    app.exit(0);
}
