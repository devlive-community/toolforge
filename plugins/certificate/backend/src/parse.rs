//! 证书解析：支持 PEM（可含多张证书的链）、Base64 DER 与二进制 DER。

use data_encoding::{BASE64, HEXUPPER};
use jiff::Timestamp;
use serde::Serialize;
use sha1::Sha1;
use sha2::{Digest, Sha256};
use tf_plugin_api::{PluginError, PluginResult};
use x509_parser::extensions::{GeneralName, ParsedExtension};
use x509_parser::oid_registry::{
    OID_KEY_TYPE_EC_PUBLIC_KEY, OID_PKCS1_RSAENCRYPTION, OID_SIG_ED25519,
};
use x509_parser::prelude::*;
use x509_parser::public_key::PublicKey;

const PEM_BEGIN: &str = "-----BEGIN CERTIFICATE-----";
const PEM_END: &str = "-----END CERTIFICATE-----";
pub const MAX_CERTIFICATES: usize = 20;

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Name {
    pub common_name: Option<String>,
    pub organization: Option<String>,
    /// RFC 4514 形式的完整名称
    pub dn: String,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct AltName {
    pub kind: &'static str,
    pub value: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Key {
    pub algorithm: String,
    pub bits: Option<usize>,
    pub curve: Option<String>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Fingerprints {
    pub sha1: String,
    pub sha256: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Certificate {
    pub subject: Name,
    pub issuer: Name,
    pub serial: String,
    pub version: u32,
    pub not_before: String,
    pub not_after: String,
    /// 距离过期的天数（已过期为负数）
    pub days_left: i64,
    /// valid / expired / not_yet_valid
    pub validity: &'static str,
    pub alt_names: Vec<AltName>,
    pub key: Key,
    pub signature_algorithm: String,
    pub self_signed: bool,
    pub is_ca: bool,
    pub path_len: Option<u32>,
    pub key_usage: Vec<String>,
    pub extended_key_usage: Vec<String>,
    pub subject_key_id: Option<String>,
    pub authority_key_id: Option<String>,
    pub ocsp: Vec<String>,
    pub ca_issuers: Vec<String>,
    pub crl: Vec<String>,
    pub fingerprints: Fingerprints,
    pub pem: String,
}

fn invalid(detail: impl std::fmt::Display) -> PluginError {
    PluginError::new("cert.invalid").with("detail", detail.to_string())
}

fn colon_hex(bytes: &[u8]) -> String {
    HEXUPPER
        .encode(bytes)
        .as_bytes()
        .chunks(2)
        .map(|pair| std::str::from_utf8(pair).unwrap_or_default())
        .collect::<Vec<_>>()
        .join(":")
}

pub fn to_pem(der: &[u8]) -> String {
    let body = BASE64.encode(der);
    let lines: Vec<&str> = body
        .as_bytes()
        .chunks(64)
        .map(|c| std::str::from_utf8(c).unwrap_or_default())
        .collect();
    format!("{PEM_BEGIN}\n{}\n{PEM_END}\n", lines.join("\n"))
}

/// 把输入拆成若干 DER 证书
pub fn split(input: &[u8]) -> PluginResult<Vec<Vec<u8>>> {
    let text = std::str::from_utf8(input).ok();
    let mut certificates = Vec::new();
    match text {
        Some(text) if text.contains(PEM_BEGIN) => {
            for block in text.split(PEM_BEGIN).skip(1) {
                let body = block
                    .split(PEM_END)
                    .next()
                    .ok_or_else(|| invalid("missing PEM end marker"))?;
                let clean: String = body.chars().filter(|c| !c.is_whitespace()).collect();
                certificates.push(BASE64.decode(clean.as_bytes()).map_err(invalid)?);
            }
        }
        Some(text) if !text.trim().is_empty() && !text.trim().starts_with('\u{30}') => {
            // 没有 PEM 头的 Base64 DER
            let clean: String = text.chars().filter(|c| !c.is_whitespace()).collect();
            certificates.push(
                BASE64
                    .decode(clean.as_bytes())
                    .map_err(|_| PluginError::new("cert.not_a_certificate"))?,
            );
        }
        _ if input.first() == Some(&0x30) => certificates.push(input.to_vec()),
        _ => return Err(PluginError::new("cert.not_a_certificate")),
    }
    if certificates.is_empty() {
        return Err(PluginError::new("cert.not_a_certificate"));
    }
    if certificates.len() > MAX_CERTIFICATES {
        return Err(PluginError::new("cert.too_many").with("max", MAX_CERTIFICATES));
    }
    Ok(certificates)
}

fn name(name: &X509Name) -> Name {
    let first = |iter: &mut dyn Iterator<Item = &AttributeTypeAndValue>| {
        iter.next()
            .and_then(|attr| attr.as_str().ok())
            .map(str::to_owned)
    };
    Name {
        common_name: first(&mut name.iter_common_name()),
        organization: first(&mut name.iter_organization()),
        dn: name.to_string(),
    }
}

fn format_time(time: ASN1Time) -> (String, i64) {
    let seconds = time.timestamp();
    let text = Timestamp::from_second(seconds)
        .map(|t| t.strftime("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|_| seconds.to_string());
    (text, seconds)
}

fn key(spki: &SubjectPublicKeyInfo) -> Key {
    let oid = &spki.algorithm.algorithm;
    let parsed = spki.parsed().ok();
    if *oid == OID_PKCS1_RSAENCRYPTION {
        let bits = match parsed {
            Some(PublicKey::RSA(rsa)) => Some(rsa.key_size()),
            _ => None,
        };
        return Key {
            algorithm: "RSA".into(),
            bits,
            curve: None,
        };
    }
    if *oid == OID_KEY_TYPE_EC_PUBLIC_KEY {
        let curve = spki
            .algorithm
            .parameters
            .as_ref()
            .and_then(|p| p.as_oid().ok())
            .map(|curve| match curve.to_id_string().as_str() {
                "1.2.840.10045.3.1.7" => "P-256".to_owned(),
                "1.3.132.0.34" => "P-384".to_owned(),
                "1.3.132.0.35" => "P-521".to_owned(),
                other => other.to_owned(),
            });
        let bits = match curve.as_deref() {
            Some("P-256") => Some(256),
            Some("P-384") => Some(384),
            Some("P-521") => Some(521),
            _ => None,
        };
        return Key {
            algorithm: "EC".into(),
            bits,
            curve,
        };
    }
    if *oid == OID_SIG_ED25519 {
        return Key {
            algorithm: "Ed25519".into(),
            bits: Some(256),
            curve: None,
        };
    }
    Key {
        algorithm: oid.to_id_string(),
        bits: None,
        curve: None,
    }
}

fn signature_name(oid: &str) -> String {
    match oid {
        "1.2.840.113549.1.1.5" => "SHA1withRSA",
        "1.2.840.113549.1.1.11" => "SHA256withRSA",
        "1.2.840.113549.1.1.12" => "SHA384withRSA",
        "1.2.840.113549.1.1.13" => "SHA512withRSA",
        "1.2.840.113549.1.1.10" => "RSASSA-PSS",
        "1.2.840.10045.4.3.2" => "SHA256withECDSA",
        "1.2.840.10045.4.3.3" => "SHA384withECDSA",
        "1.2.840.10045.4.3.4" => "SHA512withECDSA",
        "1.3.101.112" => "Ed25519",
        "1.2.156.10197.1.501" => "SM3withSM2",
        other => other,
    }
    .to_owned()
}

fn general_name(name: &GeneralName) -> Option<AltName> {
    Some(match name {
        GeneralName::DNSName(v) => AltName {
            kind: "dns",
            value: v.to_string(),
        },
        GeneralName::RFC822Name(v) => AltName {
            kind: "email",
            value: v.to_string(),
        },
        GeneralName::URI(v) => AltName {
            kind: "uri",
            value: v.to_string(),
        },
        GeneralName::IPAddress(bytes) => {
            let value = match bytes.len() {
                4 => std::net::Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]).to_string(),
                16 => {
                    let mut octets = [0u8; 16];
                    octets.copy_from_slice(bytes);
                    std::net::Ipv6Addr::from(octets).to_string()
                }
                _ => colon_hex(bytes),
            };
            AltName { kind: "ip", value }
        }
        _ => return None,
    })
}

fn uri(name: &GeneralName) -> Option<String> {
    match name {
        GeneralName::URI(v) => Some(v.to_string()),
        _ => None,
    }
}

pub fn certificate(der: &[u8], now: i64) -> PluginResult<Certificate> {
    let (_, cert) = X509Certificate::from_der(der).map_err(invalid)?;
    let (not_before, start) = format_time(cert.validity().not_before);
    let (not_after, end) = format_time(cert.validity().not_after);
    let validity = if now > end {
        "expired"
    } else if now < start {
        "not_yet_valid"
    } else {
        "valid"
    };
    let mut result = Certificate {
        subject: name(cert.subject()),
        issuer: name(cert.issuer()),
        serial: colon_hex(cert.raw_serial()),
        version: cert.version().0 + 1,
        not_before,
        not_after,
        days_left: (end - now).div_euclid(86_400),
        validity,
        alt_names: Vec::new(),
        key: key(cert.public_key()),
        signature_algorithm: signature_name(&cert.signature_algorithm.algorithm.to_id_string()),
        self_signed: cert.subject() == cert.issuer(),
        is_ca: false,
        path_len: None,
        key_usage: Vec::new(),
        extended_key_usage: Vec::new(),
        subject_key_id: None,
        authority_key_id: None,
        ocsp: Vec::new(),
        ca_issuers: Vec::new(),
        crl: Vec::new(),
        fingerprints: Fingerprints {
            sha1: colon_hex(&Sha1::digest(der)),
            sha256: colon_hex(&Sha256::digest(der)),
        },
        pem: to_pem(der),
    };
    for extension in cert.extensions() {
        match extension.parsed_extension() {
            ParsedExtension::SubjectAlternativeName(san) => {
                result.alt_names = san.general_names.iter().filter_map(general_name).collect();
            }
            ParsedExtension::BasicConstraints(bc) => {
                result.is_ca = bc.ca;
                result.path_len = bc.path_len_constraint;
            }
            ParsedExtension::KeyUsage(usage) => {
                result.key_usage = usage
                    .to_string()
                    .split(", ")
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect();
            }
            ParsedExtension::ExtendedKeyUsage(eku) => {
                let mut list = Vec::new();
                for (flag, label) in [
                    (eku.server_auth, "serverAuth"),
                    (eku.client_auth, "clientAuth"),
                    (eku.code_signing, "codeSigning"),
                    (eku.email_protection, "emailProtection"),
                    (eku.time_stamping, "timeStamping"),
                    (eku.ocsp_signing, "OCSPSigning"),
                    (eku.any, "anyExtendedKeyUsage"),
                ] {
                    if flag {
                        list.push(label.to_owned());
                    }
                }
                list.extend(eku.other.iter().map(|oid| oid.to_id_string()));
                result.extended_key_usage = list;
            }
            ParsedExtension::SubjectKeyIdentifier(id) => {
                result.subject_key_id = Some(colon_hex(id.0))
            }
            ParsedExtension::AuthorityKeyIdentifier(aki) => {
                result.authority_key_id = aki.key_identifier.as_ref().map(|id| colon_hex(id.0));
            }
            ParsedExtension::AuthorityInfoAccess(aia) => {
                for access in &aia.accessdescs {
                    let Some(location) = uri(&access.access_location) else {
                        continue;
                    };
                    match access.access_method.to_id_string().as_str() {
                        "1.3.6.1.5.5.7.48.1" => result.ocsp.push(location),
                        "1.3.6.1.5.5.7.48.2" => result.ca_issuers.push(location),
                        _ => {}
                    }
                }
            }
            ParsedExtension::CRLDistributionPoints(points) => {
                for point in points.iter() {
                    if let Some(DistributionPointName::FullName(names)) = &point.distribution_point
                    {
                        result.crl.extend(names.iter().filter_map(uri));
                    }
                }
            }
            _ => {}
        }
    }
    Ok(result)
}

/// 链中每张证书的签发者应当是下一张证书的主体
pub fn chain_ordered(certificates: &[Certificate]) -> bool {
    certificates
        .windows(2)
        .all(|pair| pair[0].issuer.dn == pair[1].subject.dn)
}

#[cfg(test)]
#[path = "parse_test.rs"]
mod tests;
