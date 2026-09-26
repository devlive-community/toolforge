//! 打开的日志文件：行索引、当前筛选视图，以及按需读取行内容。

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use encoding_rs::Encoding;
use serde::Serialize;
use tf_plugin_api::{PluginError, PluginResult, cancelled};

use crate::index::{Index, io_error};
use crate::level;
use crate::text::{self, Filter, Matcher};

/// 列表中每行最多读取的字节数与显示的字符数
const DISPLAY_BYTES: u64 = 16 * 1024;
const DISPLAY_CHARS: usize = 2000;
/// 详情中最多读取的字节数
const DETAIL_BYTES: u64 = 1024 * 1024;
const ENCODING_SAMPLE: usize = 64 * 1024;
const MAX_WORKERS: usize = 8;

pub struct View {
    pub id: u64,
    pub matcher: Arc<Matcher>,
    /// 命中的完整行
    pub lines: Vec<u32>,
    /// 尾行是否命中
    pub tail: bool,
}

impl View {
    pub fn total(&self) -> usize {
        self.lines.len() + usize::from(self.tail)
    }
}

pub struct Doc {
    pub path: PathBuf,
    pub encoding: &'static Encoding,
    pub index: RwLock<Index>,
    pub view: Mutex<Option<View>>,
    next_view: AtomicU64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    /// 行号，从 1 开始
    pub n: usize,
    pub level: &'static str,
    pub text: String,
    pub truncated: bool,
    pub marks: Vec<[usize; 2]>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub total: usize,
    pub lines: Vec<Line>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detail {
    pub n: usize,
    pub level: &'static str,
    pub text: String,
    pub truncated: bool,
    pub json: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub lines: usize,
    pub bytes: u64,
    pub counts: Counts,
}

#[derive(Debug, Serialize)]
pub struct Counts {
    pub none: u64,
    pub trace: u64,
    pub debug: u64,
    pub info: u64,
    pub warn: u64,
    pub error: u64,
}

impl From<[u64; 6]> for Counts {
    fn from(c: [u64; 6]) -> Self {
        Self {
            none: c[0],
            trace: c[1],
            debug: c[2],
            info: c[3],
            warn: c[4],
            error: c[5],
        }
    }
}

fn open_file(path: &Path) -> PluginResult<File> {
    File::open(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => {
            PluginError::new("fs.not_found").with("path", path.to_string_lossy().as_ref())
        }
        _ => io_error(e),
    })
}

fn read_range(file: &mut File, start: u64, end: u64, limit: u64) -> PluginResult<(Vec<u8>, bool)> {
    let wanted = (end - start).min(limit);
    let mut bytes = vec![0; wanted as usize];
    file.seek(SeekFrom::Start(start)).map_err(io_error)?;
    file.read_exact(&mut bytes).map_err(io_error)?;
    Ok((bytes, end - start > limit))
}

fn strip_newline(line: &mut Vec<u8>) {
    if line.last() == Some(&b'\n') {
        line.pop();
    }
    if line.last() == Some(&b'\r') {
        line.pop();
    }
}

impl Doc {
    /// 打开并建立索引；progress 回调已扫描的字节数与总字节数
    pub fn open(
        path: &Path,
        mut progress: impl FnMut(u64, u64),
        is_cancelled: impl Fn() -> bool,
    ) -> PluginResult<Self> {
        let mut file = open_file(path)?;
        let meta = file.metadata().map_err(io_error)?;
        if meta.is_dir() {
            return Err(PluginError::new("log.not_a_file"));
        }
        let mut sample = Vec::with_capacity(ENCODING_SAMPLE);
        (&mut file)
            .take(ENCODING_SAMPLE as u64)
            .read_to_end(&mut sample)
            .map_err(io_error)?;
        let encoding = text::detect_encoding(&sample)?;
        let len = meta.len();
        let mut index = Index::default();
        index.scan(&mut file, len, |done| progress(done, len), is_cancelled)?;
        Ok(Self {
            path: path.to_owned(),
            encoding,
            index: RwLock::new(index),
            view: Mutex::new(None),
            next_view: AtomicU64::new(0),
        })
    }

    pub fn stats(&self) -> Stats {
        let index = self.index.read().unwrap_or_else(|e| e.into_inner());
        Stats {
            lines: index.total(),
            bytes: index.len,
            counts: index.counts.into(),
        }
    }

    fn tail_level(&self, index: &Index, file: &mut File) -> PluginResult<u8> {
        let (start, end) = index.range(index.complete(), file)?;
        let (head, _) = read_range(file, start, end, level::HEAD as u64)?;
        let previous = index.levels.last().copied().unwrap_or(level::NONE);
        Ok(level::classify(&head, previous))
    }

    fn line_level(&self, index: &Index, line: usize, file: &mut File) -> PluginResult<u8> {
        if line < index.complete() {
            Ok(index.level(line))
        } else {
            self.tail_level(index, file)
        }
    }

    /// 在 [from, to) 的完整行中筛选，多个线程各读一段
    fn filter_range(
        &self,
        index: &Index,
        matcher: &Matcher,
        from: usize,
        to: usize,
        done: &AtomicUsize,
        is_cancelled: &(dyn Fn() -> bool + Sync),
    ) -> PluginResult<Vec<u32>> {
        let count = to.saturating_sub(from);
        if count == 0 {
            return Ok(Vec::new());
        }
        // 只按级别筛选时不需要读文件
        if !matcher.has_pattern() {
            let lines = (from..to)
                .filter(|&i| matcher.accepts_level(index.level(i)))
                .map(|i| i as u32)
                .collect();
            done.fetch_add(count, Ordering::Relaxed);
            return Ok(lines);
        }
        let workers = std::thread::available_parallelism()
            .map_or(4, |n| n.get())
            .min(MAX_WORKERS)
            .min(count.div_ceil(10_000))
            .max(1);
        let size = count.div_ceil(workers);
        let parts: Vec<PluginResult<Vec<u32>>> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..workers)
                .map(|w| {
                    let start = from + w * size;
                    let end = (start + size).min(to);
                    scope.spawn(move || -> PluginResult<Vec<u32>> {
                        let mut found = Vec::new();
                        if start >= end {
                            return Ok(found);
                        }
                        let mut file = open_file(&self.path)?;
                        file.seek(SeekFrom::Start(index.starts[start]))
                            .map_err(io_error)?;
                        let mut reader = BufReader::with_capacity(1024 * 1024, file);
                        let mut buffer = Vec::new();
                        for line in start..end {
                            if line % 4096 == 0 {
                                if is_cancelled() {
                                    return Err(cancelled());
                                }
                                done.fetch_add(4096.min(end - line), Ordering::Relaxed);
                            }
                            buffer.clear();
                            reader.read_until(b'\n', &mut buffer).map_err(io_error)?;
                            if !matcher.accepts_level(index.level(line)) {
                                continue;
                            }
                            strip_newline(&mut buffer);
                            if matcher.matches(&text::decode(&buffer, self.encoding)) {
                                found.push(line as u32);
                            }
                        }
                        Ok(found)
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| {
                    h.join()
                        .unwrap_or_else(|_| Err(PluginError::new("log.filter_failed")))
                })
                .collect()
        });
        let mut lines = Vec::new();
        for part in parts {
            lines.extend(part?);
        }
        Ok(lines)
    }

    fn tail_matches(
        &self,
        index: &Index,
        matcher: &Matcher,
        file: &mut File,
    ) -> PluginResult<bool> {
        if !index.has_tail() {
            return Ok(false);
        }
        if !matcher.accepts_level(self.tail_level(index, file)?) {
            return Ok(false);
        }
        let (start, end) = index.range(index.complete(), file)?;
        let (bytes, _) = read_range(file, start, end, DETAIL_BYTES)?;
        Ok(matcher.matches(&text::decode(&bytes, self.encoding)))
    }

    /// 建立新的筛选视图；筛选条件为空时清除视图并返回 0
    pub fn filter(
        &self,
        filter: &Filter,
        mut progress: impl FnMut(u64, u64),
        is_cancelled: &(dyn Fn() -> bool + Sync),
    ) -> PluginResult<(u64, usize)> {
        let matcher = Matcher::new(filter)?;
        if matcher.is_empty() {
            *self.view.lock().unwrap_or_else(|e| e.into_inner()) = None;
            return Ok((0, self.stats().lines));
        }
        let index = self.index.read().unwrap_or_else(|e| e.into_inner());
        let total = index.complete();
        let done = AtomicUsize::new(0);
        let lines = std::thread::scope(|scope| {
            let worker =
                scope.spawn(|| self.filter_range(&index, &matcher, 0, total, &done, is_cancelled));
            while !worker.is_finished() {
                progress(done.load(Ordering::Relaxed) as u64, total as u64);
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            worker
                .join()
                .unwrap_or_else(|_| Err(PluginError::new("log.filter_failed")))
        })?;
        let mut file = open_file(&self.path)?;
        let tail = self.tail_matches(&index, &matcher, &mut file)?;
        drop(index);
        let id = self.next_view.fetch_add(1, Ordering::Relaxed) + 1;
        let view = View {
            id,
            matcher: Arc::new(matcher),
            lines,
            tail,
        };
        let count = view.total();
        *self.view.lock().unwrap_or_else(|e| e.into_inner()) = Some(view);
        Ok((id, count))
    }

    /// 读取视图中 [offset, offset + limit) 的行；view 为 0 表示不筛选
    pub fn page(&self, view: u64, offset: usize, limit: usize) -> PluginResult<Page> {
        let index = self.index.read().unwrap_or_else(|e| e.into_inner());
        let (numbers, total, matcher) = if view == 0 {
            let total = index.total();
            let end = (offset + limit).min(total);
            ((offset.min(end)..end).collect::<Vec<_>>(), total, None)
        } else {
            let guard = self.view.lock().unwrap_or_else(|e| e.into_inner());
            let current = guard
                .as_ref()
                .filter(|v| v.id == view)
                .ok_or_else(|| PluginError::new("log.view_expired"))?;
            let total = current.total();
            let end = (offset + limit).min(total);
            let numbers = (offset.min(end)..end)
                .map(|i| {
                    current
                        .lines
                        .get(i)
                        .map_or(index.complete(), |n| *n as usize)
                })
                .collect();
            (numbers, total, Some(current.matcher.clone()))
        };
        let mut file = open_file(&self.path)?;
        let mut lines = Vec::with_capacity(numbers.len());
        for number in numbers {
            let (start, end) = index.range(number, &mut file)?;
            let (bytes, cut) = read_range(&mut file, start, end, DISPLAY_BYTES)?;
            let decoded = text::decode(&bytes, self.encoding);
            let (shown, shortened) = text::truncate(&decoded, DISPLAY_CHARS);
            lines.push(Line {
                n: number + 1,
                level: level::NAMES[self.line_level(&index, number, &mut file)? as usize],
                marks: matcher.as_ref().map(|m| m.marks(shown)).unwrap_or_default(),
                text: shown.to_owned(),
                truncated: cut || shortened,
            });
        }
        Ok(Page { total, lines })
    }

    /// 单行的完整内容（最多 1 MB）与其中的 JSON
    pub fn detail(&self, line: usize) -> PluginResult<Detail> {
        let index = self.index.read().unwrap_or_else(|e| e.into_inner());
        let number = line
            .checked_sub(1)
            .ok_or_else(|| PluginError::new("log.line_out_of_range").with("line", line))?;
        let mut file = open_file(&self.path)?;
        let (start, end) = index.range(number, &mut file)?;
        let (bytes, truncated) = read_range(&mut file, start, end, DETAIL_BYTES)?;
        let text = text::decode(&bytes, self.encoding).into_owned();
        Ok(Detail {
            n: line,
            level: level::NAMES[self.line_level(&index, number, &mut file)? as usize],
            json: text::pretty_json(&text),
            text,
            truncated,
        })
    }

    /// 文件增长时继续索引并更新视图；变小（被截断或轮转）时重新索引
    pub fn refresh(&self) -> PluginResult<Refreshed> {
        let len = std::fs::metadata(&self.path)
            .map_err(|_| {
                PluginError::new("fs.not_found").with("path", self.path.to_string_lossy().as_ref())
            })?
            .len();
        let mut file = open_file(&self.path)?;
        let (reset, before, changed) = {
            let mut index = self.index.write().unwrap_or_else(|e| e.into_inner());
            let reset = len < index.len;
            if reset {
                *index = Index::default();
            }
            let changed = len != index.len;
            let before = index.complete();
            if changed {
                index.scan(&mut file, len, |_| {}, || false)?;
            }
            (reset, before, changed || reset)
        };
        let mut view_total = None;
        let mut guard = self.view.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(view) = guard.as_mut() {
            if changed {
                let index = self.index.read().unwrap_or_else(|e| e.into_inner());
                let from = if reset { 0 } else { before };
                if reset {
                    view.lines.clear();
                }
                let done = AtomicUsize::new(0);
                let found = self.filter_range(
                    &index,
                    &view.matcher,
                    from,
                    index.complete(),
                    &done,
                    &|| false,
                )?;
                view.lines.extend(found);
                view.tail = self.tail_matches(&index, &view.matcher, &mut file)?;
            }
            view_total = Some(view.total());
        }
        drop(guard);
        Ok(Refreshed {
            changed,
            reset,
            stats: self.stats(),
            view_total,
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Refreshed {
    pub changed: bool,
    pub reset: bool,
    pub stats: Stats,
    /// 当前筛选视图的行数
    pub view_total: Option<usize>,
}

#[cfg(test)]
#[path = "doc_test.rs"]
mod tests;
