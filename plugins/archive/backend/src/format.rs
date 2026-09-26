//! 识别压缩包格式，并以统一的方式逐个访问其中的条目。

use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;
use tf_plugin_api::{PluginError, PluginResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    Zip,
    SevenZ,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
    TarZst,
    Gz,
    Bz2,
    Xz,
    Zst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stream {
    Plain,
    Gzip,
    Bzip2,
    Xz,
    Zstd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    File,
    Dir,
    /// 符号链接与硬链接：列出但不解压
    Link,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    /// 规范化后的路径（/ 分隔，目录不带结尾斜杠）
    pub path: String,
    pub kind: Kind,
    pub size: u64,
    /// 压缩后大小（tar 系列没有单独的值）
    pub packed: Option<u64>,
    /// 修改时间（本地时间文本）
    pub modified: Option<String>,
    /// 修改时间（Unix 秒），解压时用于还原
    #[serde(skip)]
    pub mtime: Option<i64>,
    pub encrypted: bool,
    /// Unix 权限位，只在 Unix 上还原
    #[serde(skip)]
    #[cfg_attr(not(unix), allow(dead_code))]
    pub mode: Option<u32>,
    /// 路径是否安全（不含 .. 且不是绝对路径）；不安全的条目只列出，不解压
    pub safe: bool,
}

pub enum Flow {
    Continue,
    Stop,
}

pub type Visitor<'a> = dyn FnMut(&Meta, Option<&mut dyn Read>) -> PluginResult<Flow> + 'a;

fn corrupt(err: impl ToString) -> PluginError {
    PluginError::new("archive.corrupt").with("detail", err.to_string())
}

fn io(err: std::io::Error) -> PluginError {
    PluginError::new("fs.io").with("detail", err.to_string())
}

/// 统计已读取的原始字节数，用于进度
struct Counting<R> {
    inner: R,
    read: Arc<AtomicU64>,
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

/// 按魔数识别外层格式
fn sniff(head: &[u8]) -> Option<Stream> {
    if head.starts_with(&[0x1f, 0x8b]) {
        Some(Stream::Gzip)
    } else if head.starts_with(b"BZh") {
        Some(Stream::Bzip2)
    } else if head.starts_with(&[0xfd, b'7', b'z', b'X', b'Z', 0x00]) {
        Some(Stream::Xz)
    } else if head.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
        Some(Stream::Zstd)
    } else {
        None
    }
}

fn is_tar(block: &[u8]) -> bool {
    block.len() >= 262 && &block[257..262] == b"ustar"
}

fn is_zip(head: &[u8]) -> bool {
    head.starts_with(b"PK\x03\x04")
        || head.starts_with(b"PK\x05\x06")
        || head.starts_with(b"PK\x07\x08")
}

const SEVEN_Z: &[u8] = &[b'7', b'z', 0xbc, 0xaf, 0x27, 0x1c];

fn read_head(path: &Path, len: usize) -> PluginResult<Vec<u8>> {
    let mut head = Vec::with_capacity(len);
    File::open(path)
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => {
                PluginError::new("fs.not_found").with("path", path.to_string_lossy().as_ref())
            }
            _ => io(e),
        })?
        .take(len as u64)
        .read_to_end(&mut head)
        .map_err(io)?;
    Ok(head)
}

fn decompress<'a>(stream: Stream, inner: impl Read + 'a) -> PluginResult<Box<dyn Read + 'a>> {
    Ok(match stream {
        Stream::Plain => Box::new(inner),
        Stream::Gzip => Box::new(flate2::read::MultiGzDecoder::new(inner)),
        Stream::Bzip2 => Box::new(bzip2::read::MultiBzDecoder::new(inner)),
        Stream::Xz => Box::new(lzma_rust2::XzReader::new(inner, true)),
        Stream::Zstd => Box::new(ruzstd::decoding::StreamingDecoder::new(inner).map_err(corrupt)?),
    })
}

/// 读取解压后开头的 512 字节以判断是否为 tar，并把它们接回流中
fn peek_block<'a>(mut reader: Box<dyn Read + 'a>) -> PluginResult<(Vec<u8>, Box<dyn Read + 'a>)> {
    let mut block = vec![0u8; 512];
    let mut filled = 0;
    while filled < block.len() {
        let n = reader.read(&mut block[filled..]).map_err(corrupt)?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    block.truncate(filled);
    let rest: Box<dyn Read + 'a> = Box::new(Cursor::new(block.clone()).chain(reader));
    Ok((block, rest))
}

pub fn detect(path: &Path) -> PluginResult<Format> {
    let head = read_head(path, 8)?;
    if is_zip(&head) {
        return Ok(Format::Zip);
    }
    if head.starts_with(SEVEN_Z) {
        return Ok(Format::SevenZ);
    }
    let stream = match sniff(&head) {
        Some(stream) => stream,
        None => {
            let block = read_head(path, 512)?;
            return if is_tar(&block) {
                Ok(Format::Tar)
            } else {
                Err(PluginError::new("archive.unsupported"))
            };
        }
    };
    let file = File::open(path).map_err(io)?;
    let (block, _) = peek_block(decompress(stream, BufReader::new(file))?)?;
    Ok(match (stream, is_tar(&block)) {
        (Stream::Gzip, true) => Format::TarGz,
        (Stream::Bzip2, true) => Format::TarBz2,
        (Stream::Xz, true) => Format::TarXz,
        (Stream::Zstd, true) => Format::TarZst,
        (Stream::Gzip, false) => Format::Gz,
        (Stream::Bzip2, false) => Format::Bz2,
        (Stream::Xz, false) => Format::Xz,
        (Stream::Zstd, false) => Format::Zst,
        (Stream::Plain, _) => Format::Tar,
    })
}

/// 规范化条目路径；包含 .. 或绝对路径的条目返回 None（解压时跳过）
pub fn normalize(raw: &str) -> Option<String> {
    let unified = raw.replace('\\', "/");
    let mut parts = Vec::new();
    for part in unified.split('/') {
        match part {
            "" | "." => {}
            ".." => return None,
            other => {
                // Windows 盘符（C:）
                if other.len() == 2 && other.ends_with(':') {
                    return None;
                }
                parts.push(other);
            }
        }
    }
    if unified.starts_with('/') && !parts.is_empty() {
        // 绝对路径：去掉开头的 / 仍然安全，但保持明确，按不安全处理
        return None;
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn local_text(seconds: i64) -> Option<String> {
    let ts = jiff::Timestamp::from_second(seconds).ok()?;
    Some(
        ts.to_zoned(jiff::tz::TimeZone::system())
            .strftime("%Y-%m-%d %H:%M")
            .to_string(),
    )
}

fn zip_time(time: zip::DateTime) -> (Option<String>, Option<i64>) {
    let civil = jiff::civil::date(time.year() as i16, time.month() as i8, time.day() as i8).at(
        time.hour() as i8,
        time.minute() as i8,
        time.second() as i8,
        0,
    );
    let text = format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        time.year(),
        time.month(),
        time.day(),
        time.hour(),
        time.minute()
    );
    let seconds = civil
        .to_zoned(jiff::tz::TimeZone::system())
        .ok()
        .map(|z| z.timestamp().as_second());
    (Some(text), seconds)
}

fn zip_error(err: zip::result::ZipError) -> PluginError {
    match err {
        zip::result::ZipError::InvalidPassword => PluginError::new("archive.wrong_password"),
        zip::result::ZipError::UnsupportedArchive(message)
            if message == zip::result::ZipError::PASSWORD_REQUIRED =>
        {
            PluginError::new("archive.password_required")
        }
        zip::result::ZipError::Io(e) => io(e),
        other => corrupt(other),
    }
}

fn seven_error(err: sevenz_rust2::Error) -> PluginError {
    match err {
        sevenz_rust2::Error::PasswordRequired => PluginError::new("archive.password_required"),
        sevenz_rust2::Error::MaybeBadPassword(_) => PluginError::new("archive.wrong_password"),
        sevenz_rust2::Error::UnsupportedCompressionMethod(method) => {
            PluginError::new("archive.unsupported_method").with("method", method)
        }
        other => corrupt(other),
    }
}

/// 逐个访问条目；data 为 true 时提供条目内容的读取器（目录与链接为 None）
pub fn walk(
    path: &Path,
    password: Option<&str>,
    data: bool,
    read: &Arc<AtomicU64>,
    visit: &mut Visitor,
) -> PluginResult<Format> {
    let format = detect(path)?;
    match format {
        Format::Zip => walk_zip(path, password, data, read, visit)?,
        Format::SevenZ => walk_seven(path, password, data, read, visit)?,
        _ => walk_stream(path, format, data, read, visit)?,
    }
    Ok(format)
}

fn walk_zip(
    path: &Path,
    password: Option<&str>,
    data: bool,
    read: &Arc<AtomicU64>,
    visit: &mut Visitor,
) -> PluginResult<()> {
    let file = Counting {
        inner: BufReader::new(File::open(path).map_err(io)?),
        read: read.clone(),
    };
    let mut archive = zip::ZipArchive::new(SeekCounting(file)).map_err(zip_error)?;
    for index in 0..archive.len() {
        let meta = {
            let entry = archive.by_index_raw(index).map_err(zip_error)?;
            let normalized = normalize(entry.name());
            let safe = normalized.is_some();
            let path = normalized.unwrap_or_else(|| entry.name().to_owned());
            let (modified, mtime) = entry.last_modified().map_or((None, None), zip_time);
            Meta {
                path,
                kind: if entry.is_dir() {
                    Kind::Dir
                } else if entry.is_symlink() {
                    Kind::Link
                } else {
                    Kind::File
                },
                size: entry.size(),
                packed: Some(entry.compressed_size()),
                modified,
                mtime,
                encrypted: entry.encrypted(),
                mode: entry.unix_mode(),
                safe,
            }
        };
        let flow = if data && meta.kind == Kind::File && meta.safe {
            let mut entry = match (meta.encrypted, password) {
                (true, Some(password)) => archive
                    .by_index_decrypt(index, password.as_bytes())
                    .map_err(zip_error)?,
                (true, None) => return Err(PluginError::new("archive.password_required")),
                (false, _) => archive.by_index(index).map_err(zip_error)?,
            };
            visit(&meta, Some(&mut entry))?
        } else {
            visit(&meta, None)?
        };
        if matches!(flow, Flow::Stop) {
            break;
        }
    }
    Ok(())
}

/// zip 需要可随机访问的读取器；计数时同样透传 Seek
struct SeekCounting<R>(Counting<R>);

impl<R: Read> Read for SeekCounting<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl<R: std::io::Seek> std::io::Seek for SeekCounting<R> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.0.inner.seek(pos)
    }
}

fn walk_seven(
    path: &Path,
    password: Option<&str>,
    data: bool,
    read: &Arc<AtomicU64>,
    visit: &mut Visitor,
) -> PluginResult<()> {
    let password = password.map_or_else(sevenz_rust2::Password::empty, sevenz_rust2::Password::new);
    let file = SeekCounting(Counting {
        inner: BufReader::new(File::open(path).map_err(io)?),
        read: read.clone(),
    });
    let mut reader = sevenz_rust2::ArchiveReader::new(file, password).map_err(seven_error)?;
    let encrypted = reader.archive().blocks.iter().any(|b| {
        b.coders
            .iter()
            .any(|c| c.encoder_method_id() == sevenz_rust2::EncoderMethod::ID_AES256_SHA256)
    });
    let meta_of = |entry: &sevenz_rust2::ArchiveEntry| -> Option<Meta> {
        let normalized = normalize(&entry.name);
        let safe = normalized.is_some();
        let path = normalized.unwrap_or_else(|| entry.name.clone());
        let mtime = entry.has_last_modified_date.then(|| {
            std::time::SystemTime::from(entry.last_modified_date)
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs() as i64)
        });
        Some(Meta {
            path,
            kind: if entry.is_directory {
                Kind::Dir
            } else {
                Kind::File
            },
            size: entry.size,
            packed: None,
            modified: mtime.and_then(local_text),
            mtime,
            encrypted,
            mode: None,
            safe,
        })
    };
    if !data {
        for entry in reader.archive().files.iter() {
            if let Some(meta) = meta_of(entry)
                && matches!(visit(&meta, None)?, Flow::Stop)
            {
                break;
            }
        }
        return Ok(());
    }
    let mut failure: Option<PluginError> = None;
    let result = reader.for_each_entries(|entry, stream| {
        let Some(meta) = meta_of(entry) else {
            return Ok(true);
        };
        let reader: Option<&mut dyn Read> = if meta.kind == Kind::File && meta.safe {
            Some(stream)
        } else {
            None
        };
        match visit(&meta, reader) {
            Ok(Flow::Continue) => Ok(true),
            Ok(Flow::Stop) => Ok(false),
            Err(err) => {
                failure = Some(err);
                Ok(false)
            }
        }
    });
    if let Some(err) = failure {
        return Err(err);
    }
    result.map_err(seven_error)
}

fn walk_stream(
    path: &Path,
    format: Format,
    data: bool,
    read: &Arc<AtomicU64>,
    visit: &mut Visitor,
) -> PluginResult<()> {
    let stream = match format {
        Format::Tar => Stream::Plain,
        Format::TarGz | Format::Gz => Stream::Gzip,
        Format::TarBz2 | Format::Bz2 => Stream::Bzip2,
        Format::TarXz | Format::Xz => Stream::Xz,
        _ => Stream::Zstd,
    };
    let file = Counting {
        inner: BufReader::new(File::open(path).map_err(io)?),
        read: read.clone(),
    };
    let mut reader = decompress(stream, file)?;
    if matches!(format, Format::Gz | Format::Bz2 | Format::Xz | Format::Zst) {
        // 单个压缩文件：条目名取文件名去掉压缩扩展名
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "data".to_owned());
        let modified = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64);
        if data {
            let meta = Meta {
                path: name,
                kind: Kind::File,
                size: 0,
                packed: None,
                modified: modified.and_then(local_text),
                mtime: modified,
                encrypted: false,
                mode: None,
                safe: true,
            };
            visit(&meta, Some(&mut reader))?;
        } else {
            // 解压一遍得到原始大小
            let size = std::io::copy(&mut reader, &mut std::io::sink()).map_err(corrupt)?;
            let meta = Meta {
                path: name,
                kind: Kind::File,
                size,
                packed: std::fs::metadata(path).ok().map(|m| m.len()),
                modified: modified.and_then(local_text),
                mtime: modified,
                encrypted: false,
                mode: None,
                safe: true,
            };
            visit(&meta, None)?;
        }
        return Ok(());
    }
    let mut archive = tar::Archive::new(reader);
    for entry in archive.entries().map_err(corrupt)? {
        let mut entry = entry.map_err(corrupt)?;
        let raw = entry.path_bytes();
        let raw = String::from_utf8_lossy(&raw).into_owned();
        let header = entry.header();
        let kind = match header.entry_type() {
            tar::EntryType::Directory => Kind::Dir,
            tar::EntryType::Regular | tar::EntryType::Continuous | tar::EntryType::GNUSparse => {
                Kind::File
            }
            tar::EntryType::Symlink | tar::EntryType::Link => Kind::Link,
            // pax 扩展头等元数据条目
            _ => continue,
        };
        let normalized = normalize(&raw);
        let safe = normalized.is_some();
        let path = normalized.unwrap_or(raw);
        let mtime = header.mtime().ok().map(|m| m as i64);
        let meta = Meta {
            path,
            kind,
            size: header.size().unwrap_or(0),
            packed: None,
            modified: mtime.and_then(local_text),
            mtime,
            encrypted: false,
            mode: header.mode().ok(),
            safe,
        };
        let flow = if data && kind == Kind::File && safe {
            visit(&meta, Some(&mut entry))?
        } else {
            visit(&meta, None)?
        };
        if matches!(flow, Flow::Stop) {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "format_test.rs"]
mod tests;
