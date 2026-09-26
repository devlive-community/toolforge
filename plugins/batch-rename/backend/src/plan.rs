//! 生成重命名计划：计算新名字，检查非法名字、批次内重名以及与已有文件的冲突。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use tf_plugin_api::PluginResult;

use crate::rules::{self, Context, Name, Step};

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Sort {
    /// 按添加顺序
    #[default]
    Added,
    /// 自然排序（file2 在 file10 之前）
    Name,
    Modified,
    Size,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanArgs {
    pub paths: Vec<String>,
    #[serde(default)]
    pub rules: Vec<Step>,
    #[serde(default)]
    pub sort: Sort,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Unchanged,
    Ok,
    Conflict,
    Invalid,
    Missing,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub path: String,
    pub from: String,
    pub to: String,
    pub status: Status,
    /// 冲突或非法的原因（错误码）
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub items: Vec<Item>,
    pub changes: usize,
    /// 冲突与非法名字的数量
    pub problems: usize,
}

/// 缓存项：读取时的修改时间与拍摄时间
type Captured = (Option<SystemTime>, Option<jiff::civil::DateTime>);

/// EXIF 拍摄时间缓存：按路径与修改时间，避免每次预览都重新读取
#[derive(Default)]
pub struct ExifCache(Mutex<HashMap<PathBuf, Captured>>);

const EXIF_EXTENSIONS: [&str; 7] = ["jpg", "jpeg", "tif", "tiff", "heic", "png", "webp"];

fn read_captured(path: &Path) -> Option<jiff::civil::DateTime> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    if !EXIF_EXTENSIONS.contains(&ext.as_str()) {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    let exif = exif::Reader::new()
        .read_from_container(&mut std::io::BufReader::new(file))
        .ok()?;
    let field = exif
        .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
        .or_else(|| exif.get_field(exif::Tag::DateTime, exif::In::PRIMARY))?;
    let exif::Value::Ascii(values) = &field.value else {
        return None;
    };
    let text = String::from_utf8_lossy(values.first()?);
    // EXIF 格式：2024:01:02 03:04:05
    jiff::civil::DateTime::strptime("%Y:%m:%d %H:%M:%S", text.trim()).ok()
}

impl ExifCache {
    fn get(&self, path: &Path, modified: Option<SystemTime>) -> Option<jiff::civil::DateTime> {
        let mut cache = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((stamp, value)) = cache.get(path)
            && *stamp == modified
        {
            return *value;
        }
        let value = read_captured(path);
        if cache.len() > 20_000 {
            cache.clear();
        }
        cache.insert(path.to_owned(), (modified, value));
        value
    }
}

struct Entry<'a> {
    path: PathBuf,
    name: Name,
    parent: String,
    modified: Option<SystemTime>,
    size: u64,
    exists: bool,
    index: usize,
    exif: &'a ExifCache,
    captured: OnceLock<Option<jiff::Zoned>>,
}

fn zoned(time: SystemTime) -> Option<jiff::Zoned> {
    jiff::Timestamp::try_from(time)
        .ok()
        .map(|t| t.to_zoned(jiff::tz::TimeZone::system()))
}

impl Context for Entry<'_> {
    fn index(&self) -> usize {
        self.index
    }
    fn original(&self) -> &Name {
        &self.name
    }
    fn parent(&self) -> &str {
        &self.parent
    }
    fn modified(&self) -> Option<jiff::Zoned> {
        self.modified.and_then(zoned)
    }
    fn captured(&self) -> Option<jiff::Zoned> {
        self.captured
            .get_or_init(|| {
                self.exif
                    .get(&self.path, self.modified)?
                    .to_zoned(jiff::tz::TimeZone::system())
                    .ok()
            })
            .clone()
    }
}

/// 自然排序：数字按数值比较，其余不区分大小写
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut digits = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        digits.push(c);
                        it.next();
                    }
                    digits
                };
                let (x, y) = (take(&mut a), take(&mut b));
                let (tx, ty) = (x.trim_start_matches('0'), y.trim_start_matches('0'));
                let order = tx
                    .len()
                    .cmp(&ty.len())
                    .then_with(|| tx.cmp(ty))
                    .then_with(|| x.len().cmp(&y.len()));
                if order.is_ne() {
                    return order;
                }
            }
            (Some(x), Some(y)) => {
                let order = x.to_lowercase().cmp(y.to_lowercase());
                if order.is_ne() {
                    return order;
                }
                a.next();
                b.next();
            }
        }
    }
}

/// macOS 与 Windows 的默认文件系统不区分大小写
pub fn path_key(path: &Path) -> String {
    let text = path.to_string_lossy();
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        text.to_lowercase()
    } else {
        text.into_owned()
    }
}

const WINDOWS_ILLEGAL: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// 检查新名字在当前系统上是否合法，返回错误码
pub fn validate(name: &str) -> Option<&'static str> {
    if name.is_empty() || name == "." || name == ".." {
        return Some("rename.empty");
    }
    if name.len() > 255 {
        return Some("rename.too_long");
    }
    if name.contains('/') || name.contains('\0') {
        return Some("rename.illegal_chars");
    }
    if cfg!(target_os = "macos") && name.contains(':') {
        return Some("rename.illegal_chars");
    }
    if cfg!(target_os = "windows") {
        if name
            .chars()
            .any(|c| WINDOWS_ILLEGAL.contains(&c) || c.is_control())
        {
            return Some("rename.illegal_chars");
        }
        if name.ends_with(' ') || name.ends_with('.') {
            return Some("rename.trailing_dot");
        }
        let stem = name
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        if WINDOWS_RESERVED.contains(&stem.as_str()) {
            return Some("rename.reserved");
        }
    }
    None
}

pub fn build(args: &PlanArgs, exif: &ExifCache) -> PluginResult<Plan> {
    let rules = rules::compile(&args.rules)?;
    let mut seen = std::collections::HashSet::new();
    let mut entries: Vec<Entry> = args
        .paths
        .iter()
        .filter(|p| seen.insert(path_key(Path::new(p))))
        .map(|raw| {
            let path = PathBuf::from(raw);
            let meta = std::fs::symlink_metadata(&path).ok();
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            Entry {
                name: Name::parse(&file_name),
                parent: path
                    .parent()
                    .and_then(Path::file_name)
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                modified: meta.as_ref().and_then(|m| m.modified().ok()),
                size: meta.as_ref().map_or(0, |m| m.len()),
                exists: meta.is_some(),
                index: 0,
                exif,
                captured: OnceLock::new(),
                path,
            }
        })
        .collect();
    match args.sort {
        Sort::Added => {}
        Sort::Name => entries.sort_by(|a, b| natural_cmp(&a.name.full(), &b.name.full())),
        Sort::Modified => entries.sort_by_key(|e| e.modified),
        Sort::Size => entries.sort_by_key(|e| e.size),
    }
    for (index, entry) in entries.iter_mut().enumerate() {
        entry.index = index;
    }

    let mut items: Vec<Item> = entries
        .iter()
        .map(|entry| {
            let from = entry.name.full();
            let to = rules::apply(&rules, entry).full();
            let (status, reason) = if !entry.exists {
                (Status::Missing, Some("fs.not_found"))
            } else if to == from {
                (Status::Unchanged, None)
            } else if let Some(code) = validate(&to) {
                (Status::Invalid, Some(code))
            } else {
                (Status::Ok, None)
            };
            Item {
                path: entry.path.to_string_lossy().into_owned(),
                from,
                to,
                status,
                reason: reason.map(str::to_owned),
            }
        })
        .collect();

    let target = |entry: &Entry, item: &Item| entry.path.with_file_name(&item.to);
    // 批次内的源文件：正在改名的会腾出位置，不变的仍占用
    let moving: HashMap<String, bool> = entries
        .iter()
        .zip(&items)
        .map(|(e, i)| (path_key(&e.path), i.status == Status::Ok))
        .collect();
    // 目标相同（包括与批次中不改名的文件同名）视为重名
    let mut targets: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, (entry, item)) in entries.iter().zip(&items).enumerate() {
        if matches!(item.status, Status::Ok | Status::Unchanged) {
            let key = path_key(&target(entry, item));
            targets.entry(key).or_default().push(index);
        }
    }
    for indexes in targets.values().filter(|v| v.len() > 1) {
        for &index in indexes {
            if items[index].status == Status::Ok {
                items[index].status = Status::Conflict;
                items[index].reason = Some("rename.duplicate".into());
            }
        }
    }
    for (entry, item) in entries.iter().zip(items.iter_mut()) {
        if item.status != Status::Ok {
            continue;
        }
        let destination = target(entry, item);
        let key = path_key(&destination);
        let same_file = key == path_key(&entry.path);
        let vacated = moving.get(&key).copied().unwrap_or(false);
        if !same_file && !vacated && std::fs::symlink_metadata(&destination).is_ok() {
            item.status = Status::Conflict;
            item.reason = Some("rename.exists".into());
        }
    }
    Ok(Plan {
        changes: items.iter().filter(|i| i.status == Status::Ok).count(),
        problems: items
            .iter()
            .filter(|i| matches!(i.status, Status::Conflict | Status::Invalid))
            .count(),
        items,
    })
}

#[cfg(test)]
#[path = "plan_test.rs"]
mod tests;
