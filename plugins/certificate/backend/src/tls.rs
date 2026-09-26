//! 获取服务器实际下发的证书链：用系统信任库校验并记录结论，但无论结果如何都完成握手，
//! 这样即使证书过期或不受信任也能查看证书内容。

use std::io::Write;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{
    CertificateError, ClientConfig, ClientConnection, DigitallySignedStruct, SignatureScheme,
};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult, TaskContext};

use crate::parse::{Certificate, certificate, chain_ordered};

const TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Deserialize)]
pub struct Args {
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_port() -> u16 {
    443
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub host: String,
    pub port: u16,
    pub address: String,
    pub protocol: Option<String>,
    pub cipher: Option<String>,
    /// trusted / expired / not_yet_valid / name_mismatch / untrusted / revoked / invalid
    pub verdict: &'static str,
    pub verdict_detail: Option<String>,
    pub certificates: Vec<Certificate>,
    pub ordered: bool,
    /// 连接地址是代理的 fake-ip（198.18.0.0/15）
    pub fake_ip: bool,
    pub elapsed_ms: u64,
}

/// 记录系统校验结果，但始终允许握手继续
#[derive(Debug)]
struct Recorder {
    inner: Arc<dyn ServerCertVerifier>,
    outcome: Mutex<Option<Result<(), rustls::Error>>>,
}

impl ServerCertVerifier for Recorder {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let result = self
            .inner
            .verify_server_cert(end_entity, intermediates, server_name, ocsp, now)
            .map(|_| ());
        *self.outcome.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}

pub fn verdict(result: &Result<(), rustls::Error>) -> (&'static str, Option<String>) {
    match result {
        Ok(()) => ("trusted", None),
        Err(rustls::Error::InvalidCertificate(err)) => {
            let verdict = match err {
                CertificateError::Expired | CertificateError::ExpiredContext { .. } => "expired",
                CertificateError::NotValidYet | CertificateError::NotValidYetContext { .. } => {
                    "not_yet_valid"
                }
                CertificateError::NotValidForName
                | CertificateError::NotValidForNameContext { .. } => "name_mismatch",
                CertificateError::UnknownIssuer => "untrusted",
                CertificateError::Revoked => "revoked",
                // 系统校验器（如 macOS）以通用错误返回原因，按关键词归类
                _ => classify(&err.to_string()),
            };
            (verdict, clean_detail(&err.to_string()))
        }
        Err(other) => (
            classify(&other.to_string()),
            clean_detail(&other.to_string()),
        ),
    }
}

/// 去掉 `Other(OtherError("…"))` 这类调试包装，只保留系统给出的说明；
/// 仅为枚举名（如 NotValidForName）时不返回，界面已有对应的文案
pub fn clean_detail(message: &str) -> Option<String> {
    let mut text = message.trim();
    for prefix in ["Other(", "OtherError(", "\""] {
        text = text.strip_prefix(prefix).unwrap_or(text);
    }
    for suffix in [")", ")", "\""] {
        text = text.strip_suffix(suffix).unwrap_or(text);
    }
    text.contains(' ').then(|| text.to_owned())
}

/// 根据系统返回的错误说明归类
pub fn classify(message: &str) -> &'static str {
    let message = message.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| message.contains(w));
    if has(&["expired"]) {
        "expired"
    } else if has(&["not yet valid", "notvalidyet"]) {
        "not_yet_valid"
    } else if has(&[
        "hostname",
        "name mismatch",
        "not valid for",
        "notvalidforname",
    ]) {
        "name_mismatch"
    } else if has(&["revoked"]) {
        "revoked"
    } else if has(&[
        "not trusted",
        "untrusted",
        "unknown issuer",
        "unknownissuer",
        "self signed",
        "self-signed",
    ]) {
        "untrusted"
    } else {
        "invalid"
    }
}

fn connect(host: &str, port: u16) -> PluginResult<(TcpStream, SocketAddr)> {
    let addresses: Vec<SocketAddr> = (host, port)
        .to_socket_addrs()
        .map_err(|_| PluginError::new("cert.resolve_failed").with("host", host))?
        .collect();
    let mut last = None;
    for address in addresses.iter().take(4) {
        match TcpStream::connect_timeout(address, TIMEOUT) {
            Ok(stream) => return Ok((stream, *address)),
            Err(err) => last = Some(err),
        }
    }
    Err(match last {
        Some(err) => PluginError::new("cert.connect_failed")
            .with("host", host)
            .with("port", port)
            .with("detail", err.to_string()),
        None => PluginError::new("cert.resolve_failed").with("host", host),
    })
}

pub fn fetch(args: Args, ctx: &dyn TaskContext) -> PluginResult<Report> {
    let start = Instant::now();
    let host = args
        .host
        .trim()
        .trim_start_matches("https://")
        .split(['/', '?'])
        .next()
        .unwrap_or_default()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_owned();
    if host.is_empty() {
        return Err(PluginError::new("cert.no_host"));
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let platform = rustls_platform_verifier::Verifier::new(provider.clone())
        .map_err(|e| PluginError::new("cert.tls_failed").with("detail", e.to_string()))?;
    let recorder = Arc::new(Recorder {
        inner: Arc::new(platform),
        outcome: Mutex::new(None),
    });
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| PluginError::new("cert.tls_failed").with("detail", e.to_string()))?
        .dangerous()
        .with_custom_certificate_verifier(recorder.clone())
        .with_no_client_auth();
    let server_name = ServerName::try_from(host.clone())
        .map_err(|_| PluginError::new("cert.invalid_host").with("host", host.as_str()))?;

    ctx.stage("cert.connect");
    let (mut socket, address) = connect(&host, args.port)?;
    let _ = socket.set_read_timeout(Some(TIMEOUT));
    let _ = socket.set_write_timeout(Some(TIMEOUT));
    let mut connection = ClientConnection::new(Arc::new(config), server_name)
        .map_err(|e| PluginError::new("cert.tls_failed").with("detail", e.to_string()))?;

    ctx.stage("cert.handshake");
    while connection.is_handshaking() {
        connection.complete_io(&mut socket).map_err(|e| {
            let code = if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut
            {
                "cert.timeout"
            } else {
                "cert.tls_failed"
            };
            PluginError::new(code).with("detail", e.to_string())
        })?;
    }
    let chain: Vec<Vec<u8>> = connection
        .peer_certificates()
        .unwrap_or_default()
        .iter()
        .map(|c| c.as_ref().to_vec())
        .collect();
    if chain.is_empty() {
        return Err(PluginError::new("cert.no_certificates"));
    }
    let protocol = connection.protocol_version().map(|v| {
        format!("{v:?}")
            .replace("TLSv1_", "TLS 1.")
            .replace("TLSv1", "TLS 1")
    });
    let cipher = connection
        .negotiated_cipher_suite()
        .map(|s| format!("{:?}", s.suite()));
    // 礼貌地关闭连接
    connection.send_close_notify();
    let _ = connection.complete_io(&mut socket);
    let _ = socket.flush();

    let now = jiff::Timestamp::now().as_second();
    let certificates = chain
        .iter()
        .map(|der| certificate(der, now))
        .collect::<PluginResult<Vec<_>>>()?;
    let outcome = recorder
        .outcome
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .unwrap_or(Ok(()));
    let (verdict, verdict_detail) = verdict(&outcome);
    Ok(Report {
        host,
        port: args.port,
        address: address.ip().to_string(),
        protocol,
        cipher,
        verdict,
        verdict_detail,
        ordered: chain_ordered(&certificates),
        fake_ip: matches!(address.ip(), std::net::IpAddr::V4(v4) if v4.octets()[0] == 198 && (v4.octets()[1] & 0xfe) == 18),
        certificates,
        elapsed_ms: start.elapsed().as_millis() as u64,
    })
}

#[cfg(test)]
#[path = "tls_test.rs"]
mod tests;
