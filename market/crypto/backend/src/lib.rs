//! 加解密插件后端：AES / SM4 / ChaCha20-Poly1305 对称加密，RSA 密钥生成、加解密与签名，
//! SM2 签名与验签调试。

mod codec;
mod rsa_ops;
mod sm2_ops;
mod symmetric;

use rand::RngCore;
use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{
    Detection, LogLevel, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, parse_args,
    to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

#[derive(Deserialize)]
struct RandomArgs {
    length: usize,
    encoding: codec::Encoding,
}

#[derive(Deserialize)]
struct KeyArgs {
    key: String,
}

pub struct Crypto {
    manifest: Manifest,
}

impl Default for Crypto {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

/// PEM 格式的 RSA / SM2 公钥或私钥；PKCS#8 / SPKI 里是 SM2 曲线时打开 SM2 页
pub fn detect_key(text: &str) -> Option<Detection> {
    let private = text.contains("-----BEGIN RSA PRIVATE KEY-----")
        || text.contains("-----BEGIN PRIVATE KEY-----");
    let public = text.contains("-----BEGIN RSA PUBLIC KEY-----")
        || text.contains("-----BEGIN PUBLIC KEY-----");
    if !private && !public {
        return None;
    }
    let sm2 = sm2_ops::parse_key(text).is_ok();
    let label = match (private, sm2) {
        (true, true) => "sm2PrivateKey",
        (true, false) => "privateKey",
        (false, true) => "sm2PublicKey",
        (false, false) => "publicKey",
    };
    Some(Detection::new(90, label))
}

impl ToolPlugin for Crypto {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn detect(&self, text: &str) -> Option<Detection> {
        detect_key(text)
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "symmetric" => to_value(symmetric::run(parse_args(args)?)?),
            "random" => {
                let args: RandomArgs = parse_args(args)?;
                if args.length == 0 || args.length > 1024 {
                    return Err(PluginError::new("crypto.invalid_length"));
                }
                let mut bytes = vec![0u8; args.length];
                rand::rngs::OsRng.fill_bytes(&mut bytes);
                Ok(Value::String(codec::encode(&bytes, args.encoding)?))
            }
            "rsa_inspect" => to_value(rsa_ops::inspect(&parse_args::<KeyArgs>(args)?.key)?),
            "rsa_encrypt" => Ok(Value::String(rsa_ops::encrypt(parse_args(args)?)?)),
            "rsa_decrypt" => Ok(Value::String(rsa_ops::decrypt(parse_args(args)?)?)),
            "rsa_sign" => Ok(Value::String(rsa_ops::sign(parse_args(args)?)?)),
            "rsa_verify" => Ok(json!({ "valid": rsa_ops::verify(parse_args(args)?)? })),
            "sm2_generate" => to_value(sm2_ops::generate()?),
            "sm2_inspect" => to_value(sm2_ops::inspect(&parse_args::<KeyArgs>(args)?.key)?),
            "sm2_export" => Ok(Value::String(sm2_ops::export_private(
                &parse_args::<KeyArgs>(args)?.key,
            )?)),
            "sm2_sign" => to_value(sm2_ops::sign(parse_args(args)?)?),
            "sm2_verify" => to_value(sm2_ops::verify(parse_args(args)?)?),
            "sm2_convert" => to_value(sm2_ops::convert(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "rsa_generate" => {
                let args: rsa_ops::GenerateArgs = parse_args(args)?;
                ctx.stage("crypto.generate");
                let start = std::time::Instant::now();
                let pair = rsa_ops::generate(args)?;
                ctx.log(
                    LogLevel::Info,
                    "crypto.generated",
                    json!({ "bits": pair.bits, "ms": start.elapsed().as_millis() as u64 }),
                );
                to_value(pair)
            }
            _ => self.call(function, args),
        }
    }
}

tf_plugin_guest::export_plugin!(Crypto);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
