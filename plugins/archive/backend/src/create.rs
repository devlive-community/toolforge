//! 创建压缩包：zip（可 AES-256 加密）、tar.gz、tar.xz、7z（可 AES-256 加密）。
//! 先写入临时文件，成功后再改名，取消或失败时不会留下半个压缩包。

use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext, cancelled};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Output {
    Zip,
    TarGz,
    TarXz,
    SevenZ,
}

fn default_level() -> u32 {
    6
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub sources: Vec<String>,
    pub output: String,
    pub format: Output,
    /// 压缩级别 0-9（0 为仅存储）
    #[serde(default = "default_level")]
    pub level: u32,
    /// zip 与 7z 支持密码（AES-256）
    #[serde(default)]
    pub password: Option<String>,
    /// 跳过 .DS_Store、Thumbs.db 等系统文件
    #[serde(default = "yes")]
    pub skip_system: bool,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub output: String,
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
    pub size: u64,
    /// 跳过的符号链接与系统文件
    pub skipped: u64,
}

struct Item {
    path: PathBuf,
    name: String,
    dir: bool,
    size: u64,
    mtime: Option<std::time::SystemTime>,
    #[cfg_attr(not(unix), allow(dead_code))]
    mode: u32,
}

const SYSTEM_FILES: [&str; 4] = [".DS_Store", "Thumbs.db", "desktop.ini", "__MACOSX"];

fn is_system(name: &str) -> bool {
    SYSTEM_FILES.contains(&name) || name.starts_with("._")
}

fn io(path: &Path, err: std::io::Error) -> PluginError {
    PluginError::new("fs.io")
        .with("path", path.to_string_lossy().as_ref())
        .with("detail", err.to_string())
}

#[cfg(unix)]
fn mode(meta: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o777
}

#[cfg(not(unix))]
fn mode(meta: &std::fs::Metadata) -> u32 {
    if meta.is_dir() { 0o755 } else { 0o644 }
}

fn collect(
    path: &Path,
    name: String,
    args: &Args,
    items: &mut Vec<Item>,
    skipped: &mut u64,
) -> PluginResult<()> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| io(path, e))?;
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if meta.file_type().is_symlink() || (args.skip_system && is_system(&file_name)) {
        *skipped += 1;
        return Ok(());
    }
    let item = Item {
        path: path.to_owned(),
        name: name.clone(),
        dir: meta.is_dir(),
        size: if meta.is_file() { meta.len() } else { 0 },
        mtime: meta.modified().ok(),
        mode: mode(&meta),
    };
    items.push(item);
    if meta.is_dir() {
        let mut children: Vec<PathBuf> = std::fs::read_dir(path)
            .map_err(|e| io(path, e))?
            .flatten()
            .map(|e| e.path())
            .collect();
        children.sort();
        for child in children {
            let child_name = format!(
                "{name}/{}",
                child
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            );
            collect(&child, child_name, args, items, skipped)?;
        }
    }
    Ok(())
}

/// 读取时统计进度并响应取消
struct Tracked<'a, R> {
    inner: R,
    done: &'a AtomicU64,
    total: u64,
    ctx: &'a dyn TaskContext,
}

impl<R: Read> Read for Tracked<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.ctx.is_cancelled() {
            return Err(std::io::Error::other("cancelled"));
        }
        let n = self.inner.read(buf)?;
        let done = self.done.fetch_add(n as u64, Ordering::Relaxed) + n as u64;
        self.ctx.progress(done, self.total);
        Ok(n)
    }
}

/// 打开源文件并包装进度统计
struct Opener<'a> {
    done: &'a AtomicU64,
    total: u64,
    ctx: &'a dyn TaskContext,
}

impl<'a> Opener<'a> {
    fn open(&self, item: &Item) -> PluginResult<Tracked<'a, File>> {
        let file = File::open(&item.path).map_err(|e| io(&item.path, e))?;
        Ok(Tracked {
            inner: file,
            done: self.done,
            total: self.total,
            ctx: self.ctx,
        })
    }
}

fn zip_time(time: Option<std::time::SystemTime>) -> Option<zip::DateTime> {
    let ts = jiff::Timestamp::try_from(time?).ok()?;
    let local = ts.to_zoned(jiff::tz::TimeZone::system());
    zip::DateTime::from_date_and_time(
        local.year() as u16,
        local.month() as u8,
        local.day() as u8,
        local.hour() as u8,
        local.minute() as u8,
        local.second() as u8,
    )
    .ok()
}

fn write_zip(file: File, items: &[Item], args: &Args, open: &Opener) -> PluginResult<()> {
    let mut writer = zip::ZipWriter::new(BufWriter::new(file));
    let method = if args.level == 0 {
        zip::CompressionMethod::Stored
    } else {
        zip::CompressionMethod::Deflated
    };
    let fail = |e: zip::result::ZipError| {
        PluginError::new("archive.write_failed").with("detail", e.to_string())
    };
    for item in items {
        let mut options = zip::write::SimpleFileOptions::default()
            .compression_method(method)
            .compression_level((args.level > 0).then_some(args.level.min(9) as i64))
            .large_file(item.size >= u32::MAX as u64);
        #[cfg(unix)]
        {
            options = options.unix_permissions(item.mode);
        }
        if let Some(time) = zip_time(item.mtime) {
            options = options.last_modified_time(time);
        }
        if item.dir {
            writer
                .add_directory(format!("{}/", item.name), options)
                .map_err(fail)?;
            continue;
        }
        let options = match args.password.as_deref().filter(|p| !p.is_empty()) {
            Some(password) => options.with_aes_encryption(zip::AesMode::Aes256, password),
            None => options,
        };
        writer
            .start_file(item.name.clone(), options)
            .map_err(fail)?;
        let mut reader = open.open(item)?;
        std::io::copy(&mut reader, &mut writer).map_err(stream_error)?;
    }
    writer
        .finish()
        .map_err(fail)?
        .flush()
        .map_err(|e| PluginError::new("fs.io").with("detail", e.to_string()))
}

fn stream_error(err: std::io::Error) -> PluginError {
    if err.to_string() == "cancelled" {
        cancelled()
    } else {
        PluginError::new("archive.write_failed").with("detail", err.to_string())
    }
}

fn write_tar<W: Write>(sink: W, items: &[Item], open: &Opener) -> PluginResult<W> {
    let mut builder = tar::Builder::new(sink);
    builder.follow_symlinks(false);
    for item in items {
        let meta = std::fs::metadata(&item.path).map_err(|e| io(&item.path, e))?;
        let mut header = tar::Header::new_gnu();
        header.set_metadata(&meta);
        if item.dir {
            header.set_size(0);
            builder
                .append_data(&mut header, format!("{}/", item.name), std::io::empty())
                .map_err(stream_error)?;
        } else {
            header.set_size(item.size);
            let reader = open.open(item)?;
            builder
                .append_data(&mut header, &item.name, reader)
                .map_err(stream_error)?;
        }
    }
    builder.into_inner().map_err(stream_error)
}

fn write_seven(file: File, items: &[Item], args: &Args, open: &Opener) -> PluginResult<()> {
    use sevenz_rust2::encoder_options::{AesEncoderOptions, Lzma2Options};
    let fail = |e: sevenz_rust2::Error| match e {
        sevenz_rust2::Error::Io(err, _) if err.to_string() == "cancelled" => cancelled(),
        other => PluginError::new("archive.write_failed").with("detail", other.to_string()),
    };
    let mut writer = sevenz_rust2::ArchiveWriter::new(BufWriter::new(file)).map_err(fail)?;
    let lzma = Lzma2Options::from_level(args.level.min(9)).into();
    match args.password.as_deref().filter(|p| !p.is_empty()) {
        Some(password) => {
            writer.set_content_methods(vec![
                AesEncoderOptions::new(sevenz_rust2::Password::new(password)).into(),
                lzma,
            ]);
        }
        None => {
            writer.set_content_methods(vec![lzma]);
        }
    }
    for item in items {
        let entry = sevenz_rust2::ArchiveEntry::from_path(&item.path, item.name.clone());
        if item.dir {
            writer
                .push_archive_entry::<std::io::Empty>(entry, None)
                .map_err(fail)?;
        } else {
            let reader = open.open(item)?;
            writer
                .push_archive_entry(entry, Some(reader))
                .map_err(fail)?;
        }
    }
    writer
        .finish()
        .map_err(stream_error)?
        .flush()
        .map_err(|e| PluginError::new("fs.io").with("detail", e.to_string()))
}

pub fn create(args: &Args, ctx: &dyn TaskContext) -> PluginResult<Outcome> {
    if args.sources.is_empty() {
        return Err(PluginError::new("archive.no_sources"));
    }
    if args.password.as_deref().is_some_and(|p| !p.is_empty())
        && matches!(args.format, Output::TarGz | Output::TarXz)
    {
        return Err(PluginError::new("archive.password_unsupported"));
    }
    let output = PathBuf::from(&args.output);
    let mut items = Vec::new();
    let mut skipped = 0;
    for source in &args.sources {
        let path = Path::new(source);
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "root".into());
        collect(path, name, args, &mut items, &mut skipped)?;
    }
    // 输出文件本身位于源文件夹中时不能打包进去
    items.retain(|item| item.path != output);
    let total: u64 = items.iter().map(|i| i.size).sum();
    let done = AtomicU64::new(0);
    let temp = output.with_file_name(format!(
        ".{}.part",
        output
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    ));
    let file = File::create(&temp).map_err(|e| io(&temp, e))?;
    let open = Opener {
        done: &done,
        total,
        ctx,
    };
    ctx.stage("archive.compress");
    let level = args.level.min(9);
    let result = match args.format {
        Output::Zip => write_zip(file, &items, args, &open),
        Output::SevenZ => write_seven(file, &items, args, &open),
        Output::TarGz => write_tar(
            flate2::write::GzEncoder::new(BufWriter::new(file), flate2::Compression::new(level)),
            &items,
            &open,
        )
        .and_then(|gz| gz.finish().map_err(stream_error))
        .and_then(|mut w| w.flush().map_err(stream_error)),
        Output::TarXz => {
            let xz = lzma_rust2::XzWriter::new(
                BufWriter::new(file),
                lzma_rust2::XzOptions::with_preset(level),
            )
            .map_err(stream_error)?;
            write_tar(xz, &items, &open)
                .and_then(|xz| xz.finish().map_err(stream_error))
                .and_then(|mut w| w.flush().map_err(stream_error))
        }
    };
    if let Err(err) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(if ctx.is_cancelled() { cancelled() } else { err });
    }
    std::fs::rename(&temp, &output).map_err(|e| io(&output, e))?;
    let outcome = Outcome {
        output: output.to_string_lossy().into_owned(),
        files: items.iter().filter(|i| !i.dir).count() as u64,
        dirs: items.iter().filter(|i| i.dir).count() as u64,
        bytes: total,
        size: std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0),
        skipped,
    };
    ctx.log(
        LogLevel::Info,
        "archive.created",
        json!({ "files": outcome.files, "bytes": outcome.bytes, "size": outcome.size }),
    );
    Ok(outcome)
}

#[cfg(test)]
#[path = "create_test.rs"]
mod tests;
