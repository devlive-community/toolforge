//! Windows：底层键盘 / 鼠标钩子（WH_KEYBOARD_LL / WH_MOUSE_LL），不需要额外授权。
//!
//! 软件模拟的输入（带 INJECTED 标志）不计入，只统计真实的键盘鼠标操作。

use std::cell::RefCell;
use std::sync::Mutex;
use std::sync::mpsc;

use tf_plugin_api::{PluginError, PluginResult};
use windows_sys::Win32::Foundation::{CloseHandle, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{
    GetCurrentThreadId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows_sys::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForMonitor, MDT_EFFECTIVE_DPI,
    SetThreadDpiAwarenessContext,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId, HC_ACTION,
    KBDLLHOOKSTRUCT, LLKHF_EXTENDED, LLKHF_INJECTED, LLMHF_INJECTED, MSG, MSLLHOOKSTRUCT,
    PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL,
    WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_MOUSEHWHEEL, WM_MOUSEMOVE,
    WM_MOUSEWHEEL, WM_QUIT, WM_RBUTTONDOWN, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_XBUTTONDOWN,
};

use super::{Permission, Sink, Stopper};
use crate::keymap;
use crate::stats::{App, Button, DEFAULT_MM_PER_PX, Display, Event, HeldKeys};

/// 钩子回调没有用户参数，事件通过全局的发送端交给聚合线程
static SINK: Mutex<Option<Sink>> = Mutex::new(None);

thread_local! {
    static HELD: RefCell<HeldKeys> = RefCell::new(HeldKeys::default());
}

const WHEEL_DELTA: f64 = 120.0;

pub fn support() -> Result<(), &'static str> {
    Ok(())
}

pub fn permission() -> Permission {
    Permission::NotRequired
}

pub fn request_permission() {}

fn send(event: Event) {
    if let Ok(sink) = SINK.lock()
        && let Some(sink) = sink.as_ref()
    {
        let _ = sink.send(event);
    }
}

/// 让坐标与显示器信息都使用物理像素
fn per_monitor_dpi() {
    unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
}

unsafe extern "system" fn keyboard(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let k = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        // AltGr 会额外产生一个扫描码超过 0xFF 的虚拟 Ctrl
        if k.flags & LLKHF_INJECTED == 0 && k.scanCode <= 0xFF {
            let key = keymap::windows(k.scanCode, k.flags & LLKHF_EXTENDED != 0);
            match wparam as u32 {
                WM_KEYDOWN | WM_SYSKEYDOWN => {
                    let pressed = HELD.with_borrow_mut(|held| held.press(key).then(|| held.mods()));
                    if let Some(mods) = pressed {
                        send(Event::Key { code: key, mods });
                    }
                }
                WM_KEYUP | WM_SYSKEYUP => HELD.with_borrow_mut(|held| held.release(key)),
                _ => {}
            }
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

unsafe extern "system" fn mouse(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let m = unsafe { &*(lparam as *const MSLLHOOKSTRUCT) };
        if m.flags & LLMHF_INJECTED == 0 {
            let (x, y) = (f64::from(m.pt.x), f64::from(m.pt.y));
            let high = (m.mouseData >> 16) as u16;
            let button = |button| Event::Button { button, x, y };
            let event = match wparam as u32 {
                WM_MOUSEMOVE => Some(Event::Move { x, y }),
                WM_LBUTTONDOWN => Some(button(Button::Left)),
                WM_RBUTTONDOWN => Some(button(Button::Right)),
                WM_MBUTTONDOWN => Some(button(Button::Middle)),
                WM_XBUTTONDOWN => Some(button(match high {
                    1 => Button::Back,
                    2 => Button::Forward,
                    _ => Button::Other,
                })),
                WM_MOUSEWHEEL | WM_MOUSEHWHEEL => Some(Event::Scroll {
                    lines: f64::from(high as i16).abs() / WHEEL_DELTA,
                }),
                _ => None,
            };
            if let Some(event) = event {
                send(event);
            }
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

pub fn start(sink: Sink) -> PluginResult<Stopper> {
    *SINK
        .lock()
        .map_err(|_| PluginError::new("input_stats.thread"))? = Some(sink);
    let (ready_tx, ready_rx) = mpsc::channel::<Option<u32>>();
    let thread = std::thread::Builder::new()
        .name("input-stats-hook".into())
        .spawn(move || {
            per_monitor_dpi();
            let module = unsafe { GetModuleHandleW(std::ptr::null()) };
            let kb = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard), module, 0) };
            let ms = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse), module, 0) };
            if kb.is_null() || ms.is_null() {
                unsafe {
                    if !kb.is_null() {
                        UnhookWindowsHookEx(kb);
                    }
                    if !ms.is_null() {
                        UnhookWindowsHookEx(ms);
                    }
                }
                let _ = ready_tx.send(None);
                return;
            }
            let _ = ready_tx.send(Some(unsafe { GetCurrentThreadId() }));
            // 底层钩子需要安装线程处理消息循环，收到 WM_QUIT 时退出
            let mut msg: MSG = unsafe { std::mem::zeroed() };
            while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {}
            unsafe {
                UnhookWindowsHookEx(kb);
                UnhookWindowsHookEx(ms);
            }
        })
        .map_err(|e| PluginError::new("input_stats.thread").with("detail", e.to_string()))?;
    match ready_rx.recv() {
        Ok(Some(thread_id)) => Ok(Box::new(move || {
            unsafe { PostThreadMessageW(thread_id, WM_QUIT, 0, 0) };
            let _ = thread.join();
            if let Ok(mut sink) = SINK.lock() {
                *sink = None;
            }
        })),
        _ => {
            let _ = thread.join();
            Err(PluginError::new("input_stats.hook_failed"))
        }
    }
}

pub fn foreground_app() -> Option<App> {
    let window = unsafe { GetForegroundWindow() };
    if window.is_null() {
        return None;
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(window, &mut pid) };
    if pid == 0 {
        return None;
    }
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return None;
    }
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    let ok = unsafe {
        QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len)
    };
    unsafe { CloseHandle(process) };
    if ok == 0 {
        return None;
    }
    let path = String::from_utf16_lossy(&buf[..len as usize]);
    let file = path.rsplit(['\\', '/']).next()?.to_owned();
    let name = file
        .rsplit_once('.')
        .map_or(file.as_str(), |(stem, _)| stem)
        .to_owned();
    Some(App {
        id: file.to_lowercase(),
        name,
    })
}

unsafe extern "system" fn collect_monitor(
    monitor: HMONITOR,
    _: HDC,
    _: *mut RECT,
    data: LPARAM,
) -> windows_sys::core::BOOL {
    let out = unsafe { &mut *(data as *mut Vec<Display>) };
    let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
    info.cbSize = size_of::<MONITORINFO>() as u32;
    if unsafe { GetMonitorInfoW(monitor, &mut info) } != 0 {
        let r = info.rcMonitor;
        let (mut dpi_x, mut dpi_y) = (0u32, 0u32);
        let mm_per_px =
            if unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) } == 0
                && dpi_x > 0
            {
                25.4 / f64::from(dpi_x)
            } else {
                DEFAULT_MM_PER_PX
            };
        out.push(Display {
            x: f64::from(r.left),
            y: f64::from(r.top),
            width: f64::from(r.right - r.left),
            height: f64::from(r.bottom - r.top),
            mm_per_px,
        });
    }
    1
}

pub fn displays() -> Vec<Display> {
    per_monitor_dpi();
    let mut out: Vec<Display> = Vec::new();
    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(collect_monitor),
            &mut out as *mut Vec<Display> as LPARAM,
        )
    };
    out
}
