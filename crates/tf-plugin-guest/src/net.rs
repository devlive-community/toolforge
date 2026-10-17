//! 插件使用的网络与文件写入能力：编译为 WASM 组件时调用宿主函数，
//! 本机构建（单元测试）时直接使用 `tf-net`，两者行为一致。
//! 插件需要在 manifest 中声明 `network`（HTTP 请求、回调监听）或 `fs:user-selected`（写文件）。

use std::time::Duration;

pub use tf_plugin_api::net::{HttpRequest, HttpResponse, NETWORK_PERMISSION};
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
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::*;

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
}

/// 由宿主发送 HTTP 请求并读取完整响应；在任务中传入 ctx 以便取消与报告进度
pub fn send(request: &HttpRequest, ctx: Option<&dyn TaskContext>) -> PluginResult<HttpResponse> {
    imp::send(request, ctx)
}

/// 写入用户选择的文件（覆盖）
pub fn write_file(path: &str, data: &[u8]) -> PluginResult<()> {
    imp::write_file(path, data)
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
