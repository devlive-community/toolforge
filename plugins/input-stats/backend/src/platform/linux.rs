//! Linux（X11）：通过 RECORD 扩展监听设备事件，不需要额外授权。
//!
//! Wayland 出于安全设计不允许应用监听全局输入，此时只能提示不支持。

use std::sync::Mutex;
use std::sync::mpsc;

use tf_plugin_api::{PluginError, PluginResult};
use x11rb::connection::{Connection, RequestConnection};
use x11rb::properties::WmClass;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::record::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{self, AtomEnum, ConnectionExt as _, KeyButMask, Window};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::x11_utils::TryParse;

use super::{Permission, Sink, Stopper};
use crate::keymap;
use crate::stats::{App, Button, DEFAULT_MM_PER_PX, Display, Event};

const RECORD_FROM_SERVER: u8 = 0;

pub fn support() -> Result<(), &'static str> {
    let wayland = std::env::var("XDG_SESSION_TYPE").is_ok_and(|t| t == "wayland")
        || std::env::var_os("WAYLAND_DISPLAY").is_some();
    if wayland {
        return Err("wayland");
    }
    if std::env::var_os("DISPLAY").is_none() {
        return Err("no_display");
    }
    Ok(())
}

pub fn permission() -> Permission {
    Permission::NotRequired
}

pub fn request_permission() {}

fn x11_error(err: impl std::fmt::Display) -> PluginError {
    PluginError::new("input_stats.x11").with("detail", err.to_string())
}

fn mods(state: KeyButMask) -> u8 {
    let mut m = 0;
    for (mask, bit) in [
        (KeyButMask::CONTROL, keymap::CTRL),
        (KeyButMask::MOD1, keymap::ALT),
        (KeyButMask::SHIFT, keymap::SHIFT),
        (KeyButMask::MOD4, keymap::META),
    ] {
        if state.contains(mask) {
            m |= bit;
        }
    }
    m
}

/// X 服务器的自动重复表现为时间戳相同的「松开 + 按下」
#[derive(Default)]
struct Repeat {
    released: Option<(u8, u32)>,
}

impl Repeat {
    fn release(&mut self, key: u8, time: u32) {
        self.released = Some((key, time));
    }

    fn is_repeat(&mut self, key: u8, time: u32) -> bool {
        self.released.take() == Some((key, time))
    }
}

/// 解析一段 RECORD 数据中的全部事件
fn parse(mut data: &[u8], repeat: &mut Repeat, sink: &Sink) {
    while !data.is_empty() {
        let kind = data[0] & 0x7f;
        let parsed = match kind {
            xproto::KEY_PRESS_EVENT | xproto::KEY_RELEASE_EVENT => {
                xproto::KeyPressEvent::try_parse(data).map(|(e, rest)| {
                    if kind == xproto::KEY_RELEASE_EVENT {
                        repeat.release(e.detail, e.time);
                    } else if !repeat.is_repeat(e.detail, e.time) {
                        let _ = sink.send(Event::Key {
                            code: keymap::x11(e.detail),
                            mods: mods(e.state),
                        });
                    }
                    rest
                })
            }
            xproto::BUTTON_PRESS_EVENT => {
                xproto::ButtonPressEvent::try_parse(data).map(|(e, rest)| {
                    let (x, y) = (f64::from(e.root_x), f64::from(e.root_y));
                    let button = match e.detail {
                        1 => Some(Button::Left),
                        2 => Some(Button::Middle),
                        3 => Some(Button::Right),
                        // 4–7 是滚轮
                        4..=7 => {
                            let _ = sink.send(Event::Scroll { lines: 1.0 });
                            None
                        }
                        8 => Some(Button::Back),
                        9 => Some(Button::Forward),
                        _ => Some(Button::Other),
                    };
                    if let Some(button) = button {
                        let _ = sink.send(Event::Button { button, x, y });
                    }
                    rest
                })
            }
            xproto::MOTION_NOTIFY_EVENT => {
                xproto::MotionNotifyEvent::try_parse(data).map(|(e, rest)| {
                    let _ = sink.send(Event::Move {
                        x: f64::from(e.root_x),
                        y: f64::from(e.root_y),
                    });
                    rest
                })
            }
            // 其他事件（松开按钮）都是 32 字节
            _ => Ok(data.get(32..).unwrap_or_default()),
        };
        match parsed {
            Ok(rest) => data = rest,
            Err(_) => return,
        }
    }
}

pub fn start(sink: Sink) -> PluginResult<Stopper> {
    support()
        .map_err(|reason| PluginError::new("input_stats.unsupported").with("reason", reason))?;
    let (ctrl, _) = x11rb::connect(None).map_err(x11_error)?;
    let (data, _) = x11rb::connect(None).map_err(x11_error)?;
    if ctrl
        .extension_information(record::X11_EXTENSION_NAME)
        .map_err(x11_error)?
        .is_none()
    {
        return Err(PluginError::new("input_stats.no_record"));
    }
    let context = ctrl.generate_id().map_err(x11_error)?;
    let empty = record::Range8 { first: 0, last: 0 };
    let empty_ext = record::ExtRange {
        major: empty,
        minor: record::Range16 { first: 0, last: 0 },
    };
    let range = record::Range {
        core_requests: empty,
        core_replies: empty,
        ext_requests: empty_ext,
        ext_replies: empty_ext,
        delivered_events: empty,
        device_events: record::Range8 {
            first: xproto::KEY_PRESS_EVENT,
            last: xproto::MOTION_NOTIFY_EVENT,
        },
        errors: empty,
        client_started: false,
        client_died: false,
    };
    ctrl.record_create_context(context, 0, &[record::CS::ALL_CLIENTS.into()], &[range])
        .map_err(x11_error)?
        .check()
        .map_err(x11_error)?;

    let (ready_tx, ready_rx) = mpsc::channel::<bool>();
    let thread = std::thread::Builder::new()
        .name("input-stats-record".into())
        .spawn(move || {
            let replies = match data.record_enable_context(context) {
                Ok(replies) => replies,
                Err(_) => {
                    let _ = ready_tx.send(false);
                    return;
                }
            };
            let _ = ready_tx.send(true);
            let mut repeat = Repeat::default();
            for reply in replies {
                let Ok(reply) = reply else { break };
                if reply.category == RECORD_FROM_SERVER && !reply.client_swapped {
                    parse(&reply.data, &mut repeat, &sink);
                }
            }
        })
        .map_err(|e| PluginError::new("input_stats.thread").with("detail", e.to_string()))?;
    if ready_rx.recv() != Ok(true) {
        let _ = thread.join();
        return Err(PluginError::new("input_stats.no_record"));
    }
    Ok(Box::new(move || {
        // 停用后数据连接上的回复流结束，线程退出
        let _ = ctrl.record_disable_context(context);
        let _ = ctrl.sync();
        let _ = thread.join();
        let _ = ctrl.record_free_context(context);
        let _ = ctrl.flush();
    }))
}

/// 查询前台应用与显示器共用的连接
struct Query {
    conn: RustConnection,
    root: Window,
    active: xproto::Atom,
}

static QUERY: Mutex<Option<Query>> = Mutex::new(None);

fn with_query<T>(f: impl FnOnce(&Query) -> Option<T>) -> Option<T> {
    let mut guard = QUERY.lock().ok()?;
    if guard.is_none() {
        let (conn, screen) = x11rb::connect(None).ok()?;
        let root = conn.setup().roots.get(screen)?.root;
        let active = conn
            .intern_atom(false, b"_NET_ACTIVE_WINDOW")
            .ok()?
            .reply()
            .ok()?
            .atom;
        *guard = Some(Query { conn, root, active });
    }
    let result = f(guard.as_ref()?);
    if result.is_none() {
        // 连接可能已断开（例如 X 服务器重启），下次重新连接
        *guard = None;
    }
    result
}

pub fn foreground_app() -> Option<App> {
    with_query(|q| {
        let reply = q
            .conn
            .get_property(false, q.root, q.active, AtomEnum::WINDOW, 0, 1)
            .ok()?
            .reply()
            .ok()?;
        let window = reply.value32()?.next()?;
        if window == 0 {
            return Some(None);
        }
        let class = WmClass::get(&q.conn, window).ok()?.reply().ok().flatten();
        let Some(class) = class else {
            return Some(None);
        };
        let name = String::from_utf8_lossy(class.class()).into_owned();
        Some(Some(App {
            id: name.to_lowercase(),
            name,
        }))
    })
    .flatten()
}

pub fn displays() -> Vec<Display> {
    with_query(|q| {
        let monitors = q.conn.randr_get_monitors(q.root, true).ok()?.reply().ok()?;
        let mut out: Vec<Display> = monitors
            .monitors
            .iter()
            .map(|m| Display {
                x: f64::from(m.x),
                y: f64::from(m.y),
                width: f64::from(m.width),
                height: f64::from(m.height),
                mm_per_px: if m.width_in_millimeters > 0 && m.width > 0 {
                    f64::from(m.width_in_millimeters) / f64::from(m.width)
                } else {
                    DEFAULT_MM_PER_PX
                },
            })
            .collect();
        if out.is_empty() {
            let screen = q.conn.setup().roots.first()?;
            out.push(Display {
                x: 0.0,
                y: 0.0,
                width: f64::from(screen.width_in_pixels),
                height: f64::from(screen.height_in_pixels),
                mm_per_px: f64::from(screen.width_in_millimeters)
                    / f64::from(screen.width_in_pixels.max(1)),
            });
        }
        Some(out)
    })
    .unwrap_or_default()
}
