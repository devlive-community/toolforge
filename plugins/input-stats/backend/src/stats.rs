//! 把输入事件聚合成计数：只累加次数，不保留事件顺序，无法还原输入内容。

use std::collections::{HashMap, HashSet};

use crate::keymap;

/// 鼠标按键
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other,
}

impl Button {
    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Middle => "middle",
            Self::Back => "back",
            Self::Forward => "forward",
            Self::Other => "other",
        }
    }
}

/// 平台采集层发出的事件；坐标为全局屏幕坐标（与 [`Display`] 同一坐标系）
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// 按下一个键（已过滤自动重复），mods 为按下时的修饰键位
    Key {
        code: &'static str,
        mods: u8,
    },
    Button {
        button: Button,
        x: f64,
        y: f64,
    },
    /// 滚动的行数（横向与纵向之和的绝对值）
    Scroll {
        lines: f64,
    },
    Move {
        x: f64,
        y: f64,
    },
}

/// 点击热力图每块屏幕横向的格数，纵向按屏幕比例计算
pub const GRID_COLS: u32 = 64;

/// 一块显示器
#[derive(Debug, Clone, PartialEq)]
pub struct Display {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// 每个坐标单位对应的物理毫米数，用于换算鼠标移动距离
    pub mm_per_px: f64,
}

impl Display {
    /// 以位置和尺寸作为标识：分辨率或排列变化后视为另一块屏幕
    pub fn id(&self) -> String {
        format!(
            "{}x{}@{},{}",
            self.width.round(),
            self.height.round(),
            self.x.round(),
            self.y.round()
        )
    }

    pub fn grid(&self) -> (u32, u32) {
        let rows = (f64::from(GRID_COLS) * self.height / self.width.max(1.0)).round();
        (GRID_COLS, (rows as u32).clamp(1, GRID_COLS * 4))
    }

    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }

    fn cell(&self, x: f64, y: f64) -> (u32, u32) {
        let (cols, rows) = self.grid();
        let col = ((x - self.x) / self.width * f64::from(cols)) as u32;
        let row = ((y - self.y) / self.height * f64::from(rows)) as u32;
        (col.min(cols - 1), row.min(rows - 1))
    }
}

/// 前台应用
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    /// 稳定标识：macOS 为 bundle id，Windows 为可执行文件名，Linux 为 WM_CLASS
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HourTotals {
    pub keys: u64,
    pub clicks: u64,
    pub scroll: f64,
    /// 鼠标移动距离（毫米）
    pub distance: f64,
    /// 活跃秒数
    pub active: u64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AppTotals {
    pub name: String,
    pub active: u64,
    pub keys: u64,
    pub clicks: u64,
}

/// 尚未写入数据库的计数，按天（本地日期 `YYYY-MM-DD`）分组
#[derive(Debug, Default, PartialEq)]
pub struct Pending {
    pub keys: HashMap<(String, &'static str), u64>,
    pub combos: HashMap<(String, String), u64>,
    pub buttons: HashMap<(String, &'static str), u64>,
    pub hours: HashMap<(String, u8), HourTotals>,
    pub clicks: HashMap<(String, String, u32, u32), u64>,
    pub displays: HashMap<String, Display>,
    pub apps: HashMap<(String, String), AppTotals>,
}

impl Pending {
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
            && self.buttons.is_empty()
            && self.hours.is_empty()
            && self.apps.is_empty()
    }
}

/// 超过这么久没有输入就不再计入活跃时间
pub const IDLE_SECS: u64 = 60;
/// 单次移动超过这个距离（坐标单位）视为光标被程序移动，不计入距离
const MAX_JUMP: f64 = 2000.0;

/// 聚合器：由采集线程按事件顺序调用，外部定时提供时钟、前台应用与显示器信息
#[derive(Debug, Default)]
pub struct Recorder {
    pending: Pending,
    day: String,
    hour: u8,
    app: Option<App>,
    displays: Vec<Display>,
    excluded: HashSet<String>,
    last_pos: Option<(f64, f64)>,
    last_input: Option<u64>,
}

impl Recorder {
    pub fn set_clock(&mut self, day: &str, hour: u8) {
        if self.day != day {
            self.day = day.to_owned();
        }
        self.hour = hour;
    }

    pub fn set_app(&mut self, app: Option<App>) {
        self.app = app;
    }

    pub fn set_displays(&mut self, displays: Vec<Display>) {
        self.displays = displays;
    }

    pub fn set_excluded(&mut self, excluded: HashSet<String>) {
        self.excluded = excluded;
    }

    /// 当前前台应用在排除名单里时完全不统计
    fn excluded_now(&self) -> bool {
        self.app
            .as_ref()
            .is_some_and(|app| self.excluded.contains(&app.id))
    }

    fn hour_totals(&mut self) -> &mut HourTotals {
        self.pending
            .hours
            .entry((self.day.clone(), self.hour))
            .or_default()
    }

    fn app_totals(&mut self) -> Option<&mut AppTotals> {
        let app = self.app.as_ref()?;
        let totals = self
            .pending
            .apps
            .entry((self.day.clone(), app.id.clone()))
            .or_default();
        if totals.name.is_empty() {
            totals.name = app.name.clone();
        }
        Some(totals)
    }

    fn display_at(&self, x: f64, y: f64) -> Option<&Display> {
        self.displays.iter().find(|d| d.contains(x, y))
    }

    /// `now` 为单调递增的秒数，只用于判断是否空闲
    pub fn record(&mut self, event: Event, now: u64) {
        if self.day.is_empty() {
            return;
        }
        if self.excluded_now() {
            if let Event::Move { x, y } | Event::Button { x, y, .. } = event {
                self.last_pos = Some((x, y));
            }
            return;
        }
        self.last_input = Some(now);
        match event {
            Event::Key { code, mods } => {
                *self
                    .pending
                    .keys
                    .entry((self.day.clone(), code))
                    .or_default() += 1;
                if let Some(combo) = keymap::combo(mods, code) {
                    *self
                        .pending
                        .combos
                        .entry((self.day.clone(), combo))
                        .or_default() += 1;
                }
                self.hour_totals().keys += 1;
                if let Some(app) = self.app_totals() {
                    app.keys += 1;
                }
            }
            Event::Button { button, x, y } => {
                *self
                    .pending
                    .buttons
                    .entry((self.day.clone(), button.name()))
                    .or_default() += 1;
                self.hour_totals().clicks += 1;
                if let Some(app) = self.app_totals() {
                    app.clicks += 1;
                }
                if let Some(display) = self.display_at(x, y).cloned() {
                    let (col, row) = display.cell(x, y);
                    let id = display.id();
                    *self
                        .pending
                        .clicks
                        .entry((self.day.clone(), id.clone(), col, row))
                        .or_default() += 1;
                    self.pending.displays.insert(id, display);
                }
                self.last_pos = Some((x, y));
            }
            Event::Scroll { lines } => self.hour_totals().scroll += lines.abs(),
            Event::Move { x, y } => {
                if let Some((px, py)) = self.last_pos {
                    let px_distance = (x - px).hypot(y - py);
                    if px_distance <= MAX_JUMP {
                        let mm = self
                            .display_at(x, y)
                            .map_or(DEFAULT_MM_PER_PX, |d| d.mm_per_px);
                        self.hour_totals().distance += px_distance * mm;
                    }
                }
                self.last_pos = Some((x, y));
            }
        }
    }

    /// 每秒调用一次：最近有输入时把经过的秒数计入当前小时与前台应用的活跃时间
    pub fn tick(&mut self, now: u64, elapsed: u64) {
        if self.day.is_empty() || elapsed == 0 || self.excluded_now() {
            return;
        }
        let active = self
            .last_input
            .is_some_and(|last| now.saturating_sub(last) <= IDLE_SECS);
        if !active {
            return;
        }
        self.hour_totals().active += elapsed;
        if let Some(app) = self.app_totals() {
            app.active += elapsed;
        }
    }

    pub fn take(&mut self) -> Pending {
        std::mem::take(&mut self.pending)
    }
}

/// 无法确定显示器物理尺寸时按 96 DPI 换算
pub const DEFAULT_MM_PER_PX: f64 = 25.4 / 96.0;

/// 过滤按住不放时系统产生的自动重复：记录哪些键处于按下状态
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Default)]
pub struct HeldKeys(HashSet<&'static str>);

#[cfg_attr(not(windows), allow(dead_code))]
impl HeldKeys {
    /// 按下：已经按着的键返回 false（自动重复）
    pub fn press(&mut self, code: &'static str) -> bool {
        self.0.insert(code)
    }

    pub fn release(&mut self, code: &'static str) {
        self.0.remove(code);
    }

    /// 当前按住的修饰键位
    pub fn mods(&self) -> u8 {
        self.0.iter().fold(0, |m, c| m | keymap::modifier_bit(c))
    }
}

#[cfg(test)]
#[path = "stats_test.rs"]
mod tests;
