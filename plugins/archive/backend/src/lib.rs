//! 压缩包插件后端：浏览、预览、解压 zip / 7z / tar（gz、bz2、xz、zst）以及单个压缩文件，
//! 并创建 zip、tar.gz、tar.xz、7z 压缩包。全部使用纯 Rust 实现。

mod catalog;
mod create;
mod extract;
mod format;

use std::collections::VecDeque;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tf_plugin_api::{
    LogLevel, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, parse_args, to_value,
    unknown_function,
};

use catalog::Catalog;
use format::{Flow, Kind, Meta};

const MANIFEST: &str = include_str!("../../manifest.json");
const CAPACITY: usize = 4;
const PREVIEW_TEXT: usize = 256 * 1024;
const PREVIEW_IMAGE: u64 = 10 * 1024 * 1024;

#[derive(Default)]
struct Catalogs {
    next_id: AtomicU64,
    items: Mutex<VecDeque<(u64, Arc<Catalog>)>>,
}

impl Catalogs {
    fn insert(&self, catalog: Catalog) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let mut items = self.items.lock().unwrap_or_else(|e| e.into_inner());
        items.push_back((id, Arc::new(catalog)));
        while items.len() > CAPACITY {
            items.pop_front();
        }
        id
    }

    fn get(&self, id: u64) -> PluginResult<Arc<Catalog>> {
        let items = self.items.lock().unwrap_or_else(|e| e.into_inner());
        items
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, c)| c.clone())
            .ok_or_else(|| PluginError::new("archive.closed"))
    }
}

#[derive(Deserialize)]
struct OpenArgs {
    path: String,
    #[serde(default)]
    password: Option<String>,
}

#[derive(Deserialize)]
struct ListArgs {
    id: u64,
    #[serde(default)]
    dir: String,
}

#[derive(Deserialize)]
struct PreviewArgs {
    id: u64,
    entry: String,
    /// 列表不需要密码的压缩包（zip）在读取内容时才需要
    #[serde(default)]
    password: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExtractArgs {
    id: u64,
    dest: String,
    #[serde(default)]
    entries: Vec<String>,
    #[serde(default)]
    conflict: extract::Conflict,
    #[serde(default)]
    create_folder: bool,
    #[serde(default)]
    password: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SuggestArgs {
    sources: Vec<String>,
    format: create::Output,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Preview {
    kind: &'static str,
    text: Option<String>,
    data_uri: Option<String>,
    truncated: bool,
    size: u64,
}

fn image_mime(name: &str) -> Option<&'static str> {
    let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        _ => return None,
    })
}

/// 看起来像文本：没有 NUL，且是合法 UTF-8（允许在末尾截断多字节字符）
fn as_text(bytes: &[u8]) -> Option<String> {
    if bytes.contains(&0) {
        return None;
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => Some(text.to_owned()),
        Err(err) if err.error_len().is_none() => {
            Some(String::from_utf8_lossy(&bytes[..err.valid_up_to()]).into_owned())
        }
        Err(_) => None,
    }
}

pub struct Archive {
    manifest: Manifest,
    catalogs: Catalogs,
}

impl Default for Archive {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
            catalogs: Catalogs::default(),
        }
    }
}

impl Archive {
    fn open(&self, args: OpenArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
        let started = Instant::now();
        let path = PathBuf::from(&args.path);
        let total = std::fs::metadata(&path)
            .map_err(|_| PluginError::new("fs.not_found").with("path", args.path.as_str()))?
            .len();
        ctx.stage("archive.read");
        let read = Arc::new(AtomicU64::new(0));
        let mut entries: Vec<Meta> = Vec::new();
        let mut visit = |meta: &Meta, _: Option<&mut dyn Read>| -> PluginResult<Flow> {
            if ctx.is_cancelled() {
                return Err(tf_plugin_api::cancelled());
            }
            if entries.len().is_multiple_of(256) {
                ctx.progress(read.load(Ordering::Relaxed), total);
            }
            entries.push(meta.clone());
            Ok(Flow::Continue)
        };
        let password = args.password.filter(|p| !p.is_empty());
        let format = format::walk(&path, password.as_deref(), false, &read, &mut visit)?;
        let catalog = Catalog::new(path.clone(), format, password, entries);
        let catalog_format = catalog.format;
        let (bytes, files) = catalog.total();
        let unsafe_paths = catalog.entries.iter().filter(|e| !e.safe).count();
        let links = catalog
            .entries
            .iter()
            .filter(|e| e.kind == Kind::Link)
            .count();
        let encrypted = catalog.encrypted();
        let elapsed_ms = started.elapsed().as_millis() as u64;
        ctx.log(
            LogLevel::Info,
            "archive.opened",
            json!({ "entries": catalog.entries.len(), "bytes": bytes, "ms": elapsed_ms }),
        );
        let id = self.catalogs.insert(catalog);
        Ok(json!({
            "id": id,
            "name": path.file_name().map(|n| n.to_string_lossy().into_owned()),
            "path": args.path,
            "format": catalog_format,
            "size": total,
            "bytes": bytes,
            "files": files,
            "encrypted": encrypted,
            "unsafePaths": unsafe_paths,
            "links": links,
            "elapsedMs": elapsed_ms,
        }))
    }

    fn preview(&self, args: PreviewArgs) -> PluginResult<Preview> {
        let catalog = self.catalogs.get(args.id)?;
        let read = Arc::new(AtomicU64::new(0));
        let mut found: Option<Preview> = None;
        let image = image_mime(&args.entry);
        let mut visit = |meta: &Meta, reader: Option<&mut dyn Read>| -> PluginResult<Flow> {
            if meta.path != args.entry {
                return Ok(Flow::Continue);
            }
            let Some(reader) = reader else {
                found = Some(Preview {
                    kind: "none",
                    text: None,
                    data_uri: None,
                    truncated: false,
                    size: meta.size,
                });
                return Ok(Flow::Stop);
            };
            let limit = if image.is_some() {
                PREVIEW_IMAGE
            } else {
                PREVIEW_TEXT as u64
            };
            let mut bytes = Vec::new();
            reader
                .take(limit + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| PluginError::new("archive.corrupt").with("detail", e.to_string()))?;
            let truncated = bytes.len() as u64 > limit;
            bytes.truncate(limit as usize);
            found = Some(match (image, truncated) {
                (Some(mime), false) => Preview {
                    kind: "image",
                    text: None,
                    data_uri: Some(format!("data:{mime};base64,{}", base64(&bytes))),
                    truncated: false,
                    size: meta.size,
                },
                (Some(_), true) => Preview {
                    kind: "tooLarge",
                    text: None,
                    data_uri: None,
                    truncated,
                    size: meta.size,
                },
                (None, _) => match as_text(&bytes) {
                    Some(text) => Preview {
                        kind: "text",
                        text: Some(text),
                        data_uri: None,
                        truncated,
                        size: meta.size,
                    },
                    None => Preview {
                        kind: "binary",
                        text: None,
                        data_uri: None,
                        truncated,
                        size: meta.size,
                    },
                },
            });
            Ok(Flow::Stop)
        };
        format::walk(
            &catalog.path,
            args.password
                .as_deref()
                .filter(|p| !p.is_empty())
                .or(catalog.password.as_deref()),
            true,
            &read,
            &mut visit,
        )?;
        found.ok_or_else(|| {
            PluginError::new("archive.entry_missing").with("entry", args.entry.clone())
        })
    }
}

/// 标准 base64（避免为预览引入额外依赖）
fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16
            | (*chunk.get(1).unwrap_or(&0) as u32) << 8
            | *chunk.get(2).unwrap_or(&0) as u32;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// 默认的输出文件：第一个源所在的文件夹，名字取单个源的名字或“Archive”，不覆盖已有文件
fn suggest(args: &SuggestArgs) -> PluginResult<String> {
    let first = args
        .sources
        .first()
        .ok_or_else(|| PluginError::new("archive.no_sources"))?;
    let first = Path::new(first);
    let dir = first.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = if args.sources.len() == 1 {
        let name = first
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if first.is_dir() {
            name
        } else {
            name.rsplit_once('.')
                .map_or(name.clone(), |(s, _)| s.to_owned())
        }
    } else {
        "Archive".to_owned()
    };
    let ext = match args.format {
        create::Output::Zip => "zip",
        create::Output::TarGz => "tar.gz",
        create::Output::TarXz => "tar.xz",
        create::Output::SevenZ => "7z",
    };
    let mut candidate = dir.join(format!("{stem}.{ext}"));
    let mut n = 1;
    while candidate.exists() {
        candidate = dir.join(format!("{stem} ({n}).{ext}"));
        n += 1;
    }
    Ok(candidate.to_string_lossy().into_owned())
}

impl ToolPlugin for Archive {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "list" => {
                let args: ListArgs = parse_args(args)?;
                let catalog = self.catalogs.get(args.id)?;
                let children = catalog.children(&args.dir).ok_or_else(|| {
                    PluginError::new("archive.entry_missing").with("entry", args.dir.clone())
                })?;
                Ok(json!({ "dir": args.dir, "children": children }))
            }
            "preview" => to_value(self.preview(parse_args(args)?)?),
            "suggest" => to_value(suggest(&parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "open" => self.open(parse_args(args)?, ctx),
            "extract" => {
                let args: ExtractArgs = parse_args(args)?;
                let catalog = self.catalogs.get(args.id)?;
                ctx.stage("archive.extract");
                let outcome = extract::extract(
                    &extract::Request {
                        archive: &catalog.path,
                        password: args
                            .password
                            .as_deref()
                            .filter(|p| !p.is_empty())
                            .or(catalog.password.as_deref()),
                        dest: Path::new(&args.dest),
                        entries: &args.entries,
                        conflict: args.conflict,
                        create_folder: args.create_folder,
                    },
                    ctx,
                )?;
                ctx.log(
                    LogLevel::Info,
                    "archive.extracted",
                    json!({ "files": outcome.files, "bytes": outcome.bytes, "dest": outcome.dest }),
                );
                to_value(outcome)
            }
            "create" => to_value(create::create(&parse_args(args)?, ctx)?),
            _ => self.call(function, args),
        }
    }
}

#[cfg(test)]
mod test_support;
