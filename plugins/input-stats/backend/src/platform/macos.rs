//! macOS：只监听（listen-only）的 CGEventTap，需要「输入监控」授权。
//!
//! 系统开启安全输入（密码框等）时，键盘事件不会送达事件监听，因此不会统计到密码输入。

use std::ffi::c_void;
use std::sync::mpsc;
use std::sync::mpsc::Sender;

use objc2::rc::autoreleasepool;
use objc2_app_kit::NSWorkspace;
use tf_plugin_api::{PluginError, PluginResult};

use super::{Permission, Sink, Stopper};
use crate::keymap;
use crate::stats::{App, Button, DEFAULT_MM_PER_PX, Display, Event};

type Ref = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

type TapCallback = extern "C" fn(proxy: Ref, kind: u32, event: Ref, user: *mut c_void) -> Ref;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        mask: u64,
        callback: TapCallback,
        user: *mut c_void,
    ) -> Ref;
    fn CGEventTapEnable(tap: Ref, enable: bool);
    fn CGEventGetIntegerValueField(event: Ref, field: u32) -> i64;
    fn CGEventGetFlags(event: Ref) -> u64;
    fn CGEventGetLocation(event: Ref) -> CGPoint;
    fn CGPreflightListenEventAccess() -> bool;
    fn CGRequestListenEventAccess() -> bool;
    fn CGGetActiveDisplayList(max: u32, displays: *mut u32, count: *mut u32) -> i32;
    fn CGDisplayBounds(display: u32) -> CGRect;
    fn CGDisplayScreenSize(display: u32) -> CGSize;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFRunLoopCommonModes: Ref;
    fn CFMachPortCreateRunLoopSource(alloc: Ref, port: Ref, order: isize) -> Ref;
    fn CFMachPortInvalidate(port: Ref);
    fn CFRunLoopGetCurrent() -> Ref;
    fn CFRunLoopAddSource(run_loop: Ref, source: Ref, mode: Ref);
    fn CFRunLoopRun();
    fn CFRunLoopStop(run_loop: Ref);
    fn CFRelease(cf: Ref);
}

// CGEventTapLocation / Placement / Options
const SESSION_TAP: u32 = 1;
const HEAD_INSERT: u32 = 0;
const LISTEN_ONLY: u32 = 1;

// CGEventType
const LEFT_DOWN: u32 = 1;
const RIGHT_DOWN: u32 = 3;
const MOUSE_MOVED: u32 = 5;
const LEFT_DRAGGED: u32 = 6;
const RIGHT_DRAGGED: u32 = 7;
const KEY_DOWN: u32 = 10;
const FLAGS_CHANGED: u32 = 12;
// NSEventTypeSystemDefined：亮度 / 音量 / 播放控制等功能层按键走这个类型
const SYSTEM_DEFINED: u32 = 14;
// NSEventSubtypeAUXControlButtons：媒体与特殊功能键
const AUX_CONTROL_BUTTONS: i16 = 8;
const SCROLL_WHEEL: u32 = 22;
const OTHER_DOWN: u32 = 25;
const OTHER_DRAGGED: u32 = 27;
const TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
const TAP_DISABLED_BY_USER: u32 = 0xFFFF_FFFF;

// CGEventField
const BUTTON_NUMBER: u32 = 3;
const KEY_AUTOREPEAT: u32 = 8;
const KEYCODE: u32 = 9;
const SCROLL_FIXED_1: u32 = 93;
const SCROLL_FIXED_2: u32 = 94;

// NSEvent 对应的私有字段：subtype 与 data1。公开字段 kCGMouseEventSubtype(7)
// 读不到这些值，映射由 macos_probe_test.rs 双向探测并作回归护栏。
const SYS_SUBTYPE: u32 = 83;
const SYS_DATA1: u32 = 149;

// CGEventFlags
const FLAG_SHIFT: u64 = 0x2_0000;
const FLAG_CONTROL: u64 = 0x4_0000;
const FLAG_ALT: u64 = 0x8_0000;
const FLAG_COMMAND: u64 = 0x10_0000;
const FLAG_FN: u64 = 0x80_0000;

const INPUT_MONITORING_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent";

pub fn support() -> Result<(), &'static str> {
    Ok(())
}

pub fn permission() -> Permission {
    if unsafe { CGPreflightListenEventAccess() } {
        Permission::Granted
    } else {
        Permission::Denied
    }
}

/// 第一次调用会弹出系统授权提示，同时打开系统设置中的「输入监控」页面
pub fn request_permission() {
    if unsafe { CGRequestListenEventAccess() } {
        return;
    }
    let _ = std::process::Command::new("open")
        .arg(INPUT_MONITORING_URL)
        .spawn();
}

struct Context {
    sink: Sink,
    tap: Ref,
}

fn mods(flags: u64) -> u8 {
    let mut m = 0;
    for (mask, bit) in [
        (FLAG_CONTROL, keymap::CTRL),
        (FLAG_ALT, keymap::ALT),
        (FLAG_SHIFT, keymap::SHIFT),
        (FLAG_COMMAND, keymap::META),
    ] {
        if flags & mask != 0 {
            m |= bit;
        }
    }
    m
}

/// 修饰键状态变化时判断是按下还是松开：查看该键自己的设备相关标志位
fn modifier_pressed(code: &str, flags: u64) -> bool {
    let mask = match code {
        "ControlLeft" => 0x1,
        "ShiftLeft" => 0x2,
        "ShiftRight" => 0x4,
        "MetaLeft" => 0x8,
        "MetaRight" => 0x10,
        "AltLeft" => 0x20,
        "AltRight" => 0x40,
        "ControlRight" => 0x2000,
        "Fn" => FLAG_FN,
        // Caps Lock 每次按下都会切换状态并产生一次事件
        _ => return true,
    };
    flags & mask != 0
}

fn location(event: Ref) -> (f64, f64) {
    let p = unsafe { CGEventGetLocation(event) };
    (p.x, p.y)
}

extern "C" fn on_event(_: Ref, kind: u32, event: Ref, user: *mut c_void) -> Ref {
    let ctx = unsafe { &*(user as *const Context) };
    let send = |e: Event| {
        let _ = ctx.sink.send(e);
    };
    match kind {
        // 回调过慢或用户输入导致系统停用监听时重新启用
        TAP_DISABLED_BY_TIMEOUT | TAP_DISABLED_BY_USER => unsafe {
            CGEventTapEnable(ctx.tap, true)
        },
        KEY_DOWN => {
            if unsafe { CGEventGetIntegerValueField(event, KEY_AUTOREPEAT) } == 0 {
                let code =
                    keymap::mac(unsafe { CGEventGetIntegerValueField(event, KEYCODE) } as u16);
                let mods = mods(unsafe { CGEventGetFlags(event) });
                send(Event::Key { code, mods });
            }
        }
        FLAGS_CHANGED => {
            let code = keymap::mac(unsafe { CGEventGetIntegerValueField(event, KEYCODE) } as u16);
            let flags = unsafe { CGEventGetFlags(event) };
            if modifier_pressed(code, flags) {
                send(Event::Key {
                    code,
                    mods: mods(flags),
                });
            }
        }
        SYSTEM_DEFINED => {
            // 功能层按键（亮度 / 音量 / 播放控制）的 subtype 与 data1 存在
            // CGEvent 私有字段里，直接读字段解析。绝对不要在这里调用
            // NSEvent::eventWithCGEvent 转换：新版 macOS 处理 Caps Lock 切换
            // 输入法的系统事件时会在转换过程中进入 TSM 输入源代码并断言主队列，
            // tap 线程上直接 SIGTRAP 整个应用（2026-10-16 所有输入框切中英文
            // 都闪退的原因）。
            if unsafe { CGEventGetIntegerValueField(event, SYS_SUBTYPE) } as i16
                != AUX_CONTROL_BUTTONS
            {
                return event;
            }
            let data1 = unsafe { CGEventGetIntegerValueField(event, SYS_DATA1) };
            // 高 16 位是键值；低 16 位里第 8 位为 0x0A 表示按下，第 0 位是自动重复
            let down = (data1 & 0xFF00) >> 8 == 0x0A;
            let repeat = data1 & 0x1 != 0;
            if down && !repeat {
                // 只统计已知功能键：私有字段读不到、或键值不在表内（包括
                // Caps Lock 切输入法的系统事件）时跳过，不产生 Unidentified 噪声
                let code = keymap::mac_special((data1 & 0xFFFF_0000) >> 16);
                if code != keymap::UNIDENTIFIED {
                    send(Event::Key { code, mods: 0 });
                }
            }
        }
        LEFT_DOWN | RIGHT_DOWN | OTHER_DOWN => {
            let button = match kind {
                LEFT_DOWN => Button::Left,
                RIGHT_DOWN => Button::Right,
                _ => match unsafe { CGEventGetIntegerValueField(event, BUTTON_NUMBER) } {
                    2 => Button::Middle,
                    3 => Button::Back,
                    4 => Button::Forward,
                    _ => Button::Other,
                },
            };
            let (x, y) = location(event);
            send(Event::Button { button, x, y });
        }
        MOUSE_MOVED | LEFT_DRAGGED | RIGHT_DRAGGED | OTHER_DRAGGED => {
            let (x, y) = location(event);
            send(Event::Move { x, y });
        }
        SCROLL_WHEEL => {
            // 16.16 定点数，触控板的连续滚动也能得到按行计的距离
            let fixed =
                |field| unsafe { CGEventGetIntegerValueField(event, field) } as f64 / 65536.0;
            let lines = fixed(SCROLL_FIXED_1).abs() + fixed(SCROLL_FIXED_2).abs();
            if lines > 0.0 {
                send(Event::Scroll { lines });
            }
        }
        _ => {}
    }
    event
}

/// 跨线程传递 CoreFoundation 指针（只在创建它的线程之外调用线程安全的 CFRunLoopStop）
struct SendRef(Ref);
unsafe impl Send for SendRef {}

impl SendRef {
    fn stop_run_loop(self) {
        unsafe { CFRunLoopStop(self.0) };
    }
}

pub fn start(sink: Sink) -> PluginResult<Stopper> {
    if permission() != Permission::Granted {
        return Err(PluginError::new("input_stats.permission"));
    }
    let (ready_tx, ready_rx) = mpsc::channel::<Option<SendRef>>();
    let thread = std::thread::Builder::new()
        .name("input-stats-tap".into())
        .spawn(move || run_tap(sink, ready_tx))
        .map_err(|e| PluginError::new("input_stats.thread").with("detail", e.to_string()))?;
    match ready_rx.recv() {
        Ok(Some(run_loop)) => Ok(Box::new(move || {
            run_loop.stop_run_loop();
            let _ = thread.join();
        })),
        _ => {
            let _ = thread.join();
            Err(PluginError::new("input_stats.permission"))
        }
    }
}

fn run_tap(sink: Sink, ready: Sender<Option<SendRef>>) {
    let mask = [
        LEFT_DOWN,
        RIGHT_DOWN,
        MOUSE_MOVED,
        LEFT_DRAGGED,
        RIGHT_DRAGGED,
        KEY_DOWN,
        FLAGS_CHANGED,
        SYSTEM_DEFINED,
        SCROLL_WHEEL,
        OTHER_DOWN,
        OTHER_DRAGGED,
    ]
    .iter()
    .fold(0u64, |m, t| m | (1 << t));
    let ctx = Box::into_raw(Box::new(Context {
        sink,
        tap: std::ptr::null_mut(),
    }));
    unsafe {
        let tap = CGEventTapCreate(
            SESSION_TAP,
            HEAD_INSERT,
            LISTEN_ONLY,
            mask,
            on_event,
            ctx.cast(),
        );
        if tap.is_null() {
            drop(Box::from_raw(ctx));
            let _ = ready.send(None);
            return;
        }
        (*ctx).tap = tap;
        let source = CFMachPortCreateRunLoopSource(std::ptr::null_mut(), tap, 0);
        let run_loop = CFRunLoopGetCurrent();
        CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);
        let _ = ready.send(Some(SendRef(run_loop)));
        CFRunLoopRun();
        CGEventTapEnable(tap, false);
        CFMachPortInvalidate(tap);
        CFRelease(source);
        CFRelease(tap);
        drop(Box::from_raw(ctx));
    }
}

pub fn foreground_app() -> Option<App> {
    autoreleasepool(|_| {
        let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
        let name = app.localizedName().map(|s| s.to_string());
        let id = app.bundleIdentifier().map(|s| s.to_string());
        let id = id.or_else(|| name.clone())?;
        Some(App {
            name: name.unwrap_or_else(|| id.clone()),
            id,
        })
    })
}

pub fn displays() -> Vec<Display> {
    let mut ids = [0u32; 16];
    let mut count = 0u32;
    if unsafe { CGGetActiveDisplayList(ids.len() as u32, ids.as_mut_ptr(), &mut count) } != 0 {
        return Vec::new();
    }
    ids[..count as usize]
        .iter()
        .map(|&id| {
            let bounds = unsafe { CGDisplayBounds(id) };
            let size = unsafe { CGDisplayScreenSize(id) };
            let mm_per_px = if size.width > 0.0 && bounds.size.width > 0.0 {
                size.width / bounds.size.width
            } else {
                DEFAULT_MM_PER_PX
            };
            Display {
                x: bounds.origin.x,
                y: bounds.origin.y,
                width: bounds.size.width,
                height: bounds.size.height,
                mm_per_px,
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "macos_probe_test.rs"]
mod macos_probe_test;
