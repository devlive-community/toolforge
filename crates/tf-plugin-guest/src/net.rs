//! 插件使用的网络与文件写入能力：编译为 WASM 组件时调用宿主函数，
//! 本机构建（单元测试）时直接使用 `tf-net`，两者行为一致。包括一次性请求、流式请求、回调监听与写文件。
//! 插件需要在 manifest 中声明 `network`（HTTP 请求、回调监听）或 `fs:user-selected`（写文件）。

use std::time::Duration;

pub use tf_plugin_api::net::{
    HttpRequest, HttpResponse, NETWORK_PERMISSION, StreamEvent, StreamHead, StreamRequest,
};
use tf_plugin_api::{PluginResult, TaskContext};

#[cfg(target_arch = "wasm32")]
mod imp {
    use super::*;
    use crate::bindings::toolforge::plugin::host;
    use tf_plugin_api::PluginError;

    fn error(f: host::Failure) -> PluginError {
        let mut err = PluginError::new(f.code);
        err.params = serde_json::from_str(&f.params).unwrap_or_default();
        err
    }

    /// 宿主在任务中自行检查取消、报告进度
    pub fn send(
        request: &HttpRequest,
        _ctx: Option<&dyn TaskContext>,
    ) -> PluginResult<HttpResponse> {
        let response = host::http_send(&host::HttpRequest {
            method: request.method.clone(),
            url: request.url.clone(),
            headers: request.headers.clone(),
            body: request.body.clone(),
            timeout_ms: request.timeout_ms,
            follow_redirects: request.follow_redirects,
            max_redirects: request.max_redirects,
            use_proxy: request.use_proxy,
            max_body: request.max_body,
        })
        .map_err(error)?;
        Ok(HttpResponse {
            status: response.status,
            reason: response.reason,
            version: response.version,
            url: response.url,
            headers: response.headers,
            body: response.body,
            redirects: response.redirects,
            elapsed_ms: response.elapsed_ms,
        })
    }

    pub struct Listener(u32);

    impl Listener {
        pub fn listen(port: u16, path: &str, timeout: Duration) -> PluginResult<(Self, u16)> {
            let millis = timeout.as_millis().min(u128::from(u32::MAX)) as u32;
            let (id, port) = host::callback_listen(port, path, millis).map_err(error)?;
            Ok((Self(id), port))
        }

        pub fn poll(&self) -> PluginResult<Option<String>> {
            host::callback_poll(self.0).map_err(error)
        }
    }

    impl Drop for Listener {
        fn drop(&mut self) {
            host::callback_close(self.0);
        }
    }

    pub fn write_file(path: &str, data: &[u8]) -> PluginResult<()> {
        host::write_file(path, data).map_err(error)
    }

    pub fn file_size(path: &str) -> PluginResult<u64> {
        host::file_size(path).map_err(error)
    }

    pub fn read_file(path: &str, offset: u64, length: u32) -> PluginResult<Vec<u8>> {
        host::read_file(path, offset, length).map_err(error)
    }

    pub struct Streaming(u32);

    impl Streaming {
        pub fn open(
            request: &StreamRequest,
            _ctx: Option<&dyn TaskContext>,
        ) -> PluginResult<(Self, StreamHead)> {
            let (id, head) = host::http_stream_open(&host::StreamRequest {
                method: request.method.clone(),
                url: request.url.clone(),
                headers: request.headers.clone(),
                body: request.body.clone(),
                timeout_ms: request.timeout_ms,
                http2_prior_knowledge: request.http2_prior_knowledge,
            })
            .map_err(error)?;
            Ok((
                Self(id),
                StreamHead {
                    status: head.status,
                    reason: head.reason,
                    version: head.version,
                    headers: head.headers,
                },
            ))
        }

        pub fn read(&mut self, wait: Duration) -> PluginResult<StreamEvent> {
            let millis = wait.as_millis().min(u128::from(u32::MAX)) as u32;
            Ok(
                match host::http_stream_read(self.0, millis).map_err(error)? {
                    host::StreamEvent::Data(data) => StreamEvent::Data(data),
                    host::StreamEvent::Pending => StreamEvent::Pending,
                    host::StreamEvent::End(trailers) => StreamEvent::End(trailers),
                },
            )
        }
    }

    impl Drop for Streaming {
        fn drop(&mut self) {
            host::http_stream_close(self.0);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::*;
    use tf_plugin_api::PluginError;

    pub fn send(
        request: &HttpRequest,
        ctx: Option<&dyn TaskContext>,
    ) -> PluginResult<HttpResponse> {
        tf_net::send(
            request,
            &|| ctx.is_some_and(|c| c.is_cancelled()),
            &|done, total| {
                if let Some(c) = ctx {
                    c.progress(done, total);
                }
            },
        )
    }

    pub struct Listener(tf_net::CallbackServer);

    impl Listener {
        pub fn listen(port: u16, path: &str, timeout: Duration) -> PluginResult<(Self, u16)> {
            let server = tf_net::CallbackServer::listen(port, path, timeout)?;
            let port = server.port();
            Ok((Self(server), port))
        }

        pub fn poll(&self) -> PluginResult<Option<String>> {
            self.0.poll()
        }
    }

    pub fn write_file(path: &str, data: &[u8]) -> PluginResult<()> {
        tf_net::write_file(path, data)
    }

    pub fn file_size(path: &str) -> PluginResult<u64> {
        let meta = std::fs::metadata(path).map_err(|e| {
            PluginError::new("fs.io")
                .with("path", path)
                .with("detail", e.to_string())
        })?;
        if meta.is_dir() {
            return Err(PluginError::new("fs.not_a_file").with("path", path));
        }
        Ok(meta.len())
    }

    pub fn read_file(path: &str, offset: u64, length: u32) -> PluginResult<Vec<u8>> {
        use std::io::{Read, Seek, SeekFrom};
        let mut file = std::fs::File::open(path).map_err(|e| {
            PluginError::new("fs.io")
                .with("path", path)
                .with("detail", e.to_string())
        })?;
        file.seek(SeekFrom::Start(offset)).map_err(|e| {
            PluginError::new("fs.io")
                .with("path", path)
                .with("detail", e.to_string())
        })?;
        let mut buf = Vec::with_capacity(length as usize);
        file.take(u64::from(length))
            .read_to_end(&mut buf)
            .map_err(|e| {
                PluginError::new("fs.io")
                    .with("path", path)
                    .with("detail", e.to_string())
            })?;
        Ok(buf)
    }

    pub struct Streaming(tf_net::HttpStream);

    impl Streaming {
        pub fn open(
            request: &StreamRequest,
            ctx: Option<&dyn TaskContext>,
        ) -> PluginResult<(Self, StreamHead)> {
            let (stream, head) =
                tf_net::HttpStream::open(request, &|| ctx.is_some_and(|c| c.is_cancelled()))?;
            Ok((Self(stream), head))
        }

        pub fn read(&mut self, wait: Duration) -> PluginResult<StreamEvent> {
            self.0.read(wait)
        }
    }
}

/// 由宿主发送 HTTP 请求并读取完整响应；在任务中传入 ctx 以便取消与报告进度
pub fn send(request: &HttpRequest, ctx: Option<&dyn TaskContext>) -> PluginResult<HttpResponse> {
    imp::send(request, ctx)
}

/// 写入用户选择的文件（覆盖）
pub fn write_file(path: &str, data: &[u8]) -> PluginResult<()> {
    imp::write_file(path, data)
}

/// 读取用户选择的文件大小（需要 `fs:user-selected` 权限）
pub fn file_size(path: &str) -> PluginResult<u64> {
    imp::file_size(path)
}

/// 读取用户选择文件中 [offset, offset + length) 的字节
pub fn read_file(path: &str, offset: u64, length: u32) -> PluginResult<Vec<u8>> {
    imp::read_file(path, offset, length)
}

/// 127.0.0.1 上的一次性回调监听，drop 时关闭并释放端口
pub struct Callback {
    inner: imp::Listener,
    port: u16,
}

impl Callback {
    /// 端口为 0 时由系统分配；只接受第一个匹配路径的 GET 请求
    pub fn listen(port: u16, path: &str, timeout: Duration) -> PluginResult<Self> {
        let (inner, port) = imp::Listener::listen(port, path, timeout)?;
        Ok(Self { inner, port })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// 收到回调时返回查询字符串；还在等待时为 None；超时返回 `net.callback_timeout`
    pub fn poll(&self) -> PluginResult<Option<String>> {
        self.inner.poll()
    }
}

/// 流式 HTTP 请求：逐块读取响应体，结束时拿到 trailers；drop 时中止
pub struct Stream {
    inner: imp::Streaming,
    head: StreamHead,
}

impl Stream {
    /// 连接并发送请求，拿到响应头；在任务中传入 ctx，等待响应头期间也能取消
    pub fn open(request: &StreamRequest, ctx: Option<&dyn TaskContext>) -> PluginResult<Self> {
        let (inner, head) = imp::Streaming::open(request, ctx)?;
        Ok(Self { inner, head })
    }

    pub fn head(&self) -> &StreamHead {
        &self.head
    }

    /// 最多等待 `wait`：一块响应体、`Pending` 或 `End`（带 trailers）
    pub fn read(&mut self, wait: Duration) -> PluginResult<StreamEvent> {
        self.inner.read(wait)
    }
}
