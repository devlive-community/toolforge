use std::io::Read;
use std::net::TcpListener;
use std::path::PathBuf;

use rustls::ServerConfig;
use rustls::pki_types::PrivateKeyDer;
use serde_json::Value;
use tf_plugin_api::LogLevel;

use super::*;

struct Ctx;

impl TaskContext for Ctx {
    fn log(&self, _: LogLevel, _: &str, _: Value) {}
    fn progress(&self, _: u64, _: u64) {}
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        false
    }
    fn open_file(&self, _: &str) -> PluginResult<Box<dyn Read + Send>> {
        Err(PluginError::new("fs.not_found"))
    }
    fn file_size(&self, _: &str) -> PluginResult<u64> {
        Err(PluginError::new("fs.not_found"))
    }
    fn resource_path(&self, id: &str) -> PluginResult<PathBuf> {
        Err(PluginError::new("resource.missing").with("id", id))
    }
}

/// 在本机启动一个使用自签名证书的 TLS 服务，接受一次连接
fn serve_self_signed() -> u16 {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let key = PrivateKeyDer::Pkcs8(cert.signing_key.serialize_der().into());
    let config =
        ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![cert.cert.der().clone()], key)
            .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut connection = rustls::ServerConnection::new(Arc::new(config)).unwrap();
        while connection.is_handshaking() {
            if connection.complete_io(&mut socket).is_err() {
                return;
            }
        }
        let _ = connection.complete_io(&mut socket);
    });
    port
}

#[test]
fn captures_untrusted_chains() {
    let port = serve_self_signed();
    let report = fetch(
        Args {
            host: "localhost".into(),
            port,
        },
        &Ctx,
    )
    .unwrap();
    assert_eq!(report.verdict, "untrusted", "{report:?}");
    assert_eq!(report.certificates.len(), 1);
    assert!(report.certificates[0].self_signed);
    assert_eq!(report.protocol.as_deref(), Some("TLS 1.3"));
    assert!(report.cipher.is_some());
}

#[test]
fn maps_verification_errors() {
    let err = |e| Err(rustls::Error::InvalidCertificate(e));
    assert_eq!(verdict(&Ok(())).0, "trusted");
    assert_eq!(verdict(&err(CertificateError::Expired)).0, "expired");
    assert_eq!(
        verdict(&err(CertificateError::NotValidForName)).0,
        "name_mismatch"
    );
    assert_eq!(
        verdict(&err(CertificateError::UnknownIssuer)).0,
        "untrusted"
    );
    assert_eq!(verdict(&err(CertificateError::BadSignature)).0, "invalid");
    assert_eq!(
        classify("“x” certificate is not trusted: -67843"),
        "untrusted"
    );
    assert_eq!(classify("certificate has expired"), "expired");
    assert_eq!(classify("Hostname mismatch"), "name_mismatch");
    assert_eq!(classify("something else"), "invalid");
    assert_eq!(
        clean_detail("Other(OtherError(\"“x” certificate is expired: -67818\"))").as_deref(),
        Some("“x” certificate is expired: -67818")
    );
    assert_eq!(clean_detail("NotValidForName"), None);
}

#[test]
fn reports_connection_problems() {
    let closed = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let err = fetch(
        Args {
            host: "127.0.0.1".into(),
            port: closed,
        },
        &Ctx,
    )
    .unwrap_err();
    assert_eq!(err.code, "cert.connect_failed");
    assert_eq!(
        fetch(
            Args {
                host: "  ".into(),
                port: 443
            },
            &Ctx
        )
        .unwrap_err()
        .code,
        "cert.no_host"
    );
}

/// 需要网络：`cargo test -p tfp-certificate -- --ignored`
#[test]
#[ignore]
fn fetches_a_public_chain() {
    let report = fetch(
        Args {
            host: "https://github.com/devlive".into(),
            port: 443,
        },
        &Ctx,
    )
    .unwrap();
    println!(
        "{} {:?} {:?} {}",
        report.verdict,
        report.protocol,
        report.cipher,
        report.certificates.len()
    );
    assert_eq!(report.host, "github.com");
    assert!(report.certificates.len() >= 2);
}
