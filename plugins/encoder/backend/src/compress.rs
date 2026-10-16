//! 字节级压缩与解压：gzip / deflate（raw）/ zlib / zstd。

use std::io::{Read, Write};

use tf_plugin_api::{PluginError, PluginResult};

use crate::codec::Codec;

/// 解压结果上限，防止压缩炸弹撑爆内存
pub const MAX_OUTPUT: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Format {
    Gzip,
    Deflate,
    Zlib,
    Zstd,
}

impl Format {
    pub fn of(codec: Codec) -> Self {
        match codec {
            Codec::Gzip => Format::Gzip,
            Codec::Deflate => Format::Deflate,
            Codec::Zlib => Format::Zlib,
            _ => Format::Zstd,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Format::Gzip => "gzip",
            Format::Deflate => "deflate",
            Format::Zlib => "zlib",
            Format::Zstd => "zstd",
        }
    }
}

fn failed(format: Format, e: impl ToString) -> PluginError {
    PluginError::new("encode.compress_failed")
        .with("codec", format.name())
        .with("detail", e.to_string())
}

pub fn compress(format: Format, input: &[u8]) -> PluginResult<Vec<u8>> {
    let level = flate2::Compression::default();
    match format {
        Format::Gzip => {
            let mut encoder = flate2::write::GzEncoder::new(Vec::new(), level);
            encoder.write_all(input).map_err(|e| failed(format, e))?;
            encoder.finish().map_err(|e| failed(format, e))
        }
        Format::Deflate => {
            let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), level);
            encoder.write_all(input).map_err(|e| failed(format, e))?;
            encoder.finish().map_err(|e| failed(format, e))
        }
        Format::Zlib => {
            let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), level);
            encoder.write_all(input).map_err(|e| failed(format, e))?;
            encoder.finish().map_err(|e| failed(format, e))
        }
        Format::Zstd => Ok(ruzstd::encoding::compress_to_vec(
            input,
            ruzstd::encoding::CompressionLevel::Fastest,
        )),
    }
}

/// 记录已读取字节数的读取器，用于在数据损坏时报告位置
struct Counting<'a> {
    data: &'a [u8],
    read: std::rc::Rc<std::cell::Cell<usize>>,
}

impl Read for Counting<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.data.len().min(buf.len());
        buf[..n].copy_from_slice(&self.data[..n]);
        self.data = &self.data[n..];
        self.read.set(self.read.get() + n);
        Ok(n)
    }
}

pub fn decompress(format: Format, input: &[u8]) -> PluginResult<Vec<u8>> {
    let consumed = std::rc::Rc::new(std::cell::Cell::new(0));
    let source = Counting {
        data: input,
        read: consumed.clone(),
    };
    let reader: Box<dyn Read> = match format {
        Format::Gzip => Box::new(flate2::read::MultiGzDecoder::new(source)),
        Format::Deflate => Box::new(flate2::read::DeflateDecoder::new(source)),
        Format::Zlib => Box::new(flate2::read::ZlibDecoder::new(source)),
        Format::Zstd => Box::new(
            ruzstd::decoding::StreamingDecoder::new(source).map_err(|e| {
                PluginError::new("encode.corrupt")
                    .with("codec", format.name())
                    .with("offset", 0)
                    .with("detail", e.to_string())
            })?,
        ),
    };
    let mut out = Vec::new();
    reader
        .take(MAX_OUTPUT + 1)
        .read_to_end(&mut out)
        .map_err(|e| {
            PluginError::new("encode.corrupt")
                .with("codec", format.name())
                .with("offset", consumed.get())
                .with("detail", e.to_string())
        })?;
    if out.len() as u64 > MAX_OUTPUT {
        return Err(PluginError::new("encode.output_too_large").with("limit", "64 MB"));
    }
    if out.is_empty() && !input.is_empty() && format != Format::Zstd {
        // flate2 遇到不完整的流可能返回空结果而不报错
        return Err(PluginError::new("encode.corrupt")
            .with("codec", format.name())
            .with("offset", consumed.get())
            .with("detail", "no data decoded".to_owned()));
    }
    Ok(out)
}

#[cfg(test)]
#[path = "compress_test.rs"]
mod tests;
