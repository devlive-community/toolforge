//! 用标准校验器检查模块：格式、section 顺序、索引范围、函数类型规则。
//!
//! 先按默认特性集（WebAssembly 3.0 草案及已标准化的提案）校验；失败时再打开全部提案重试，
//! 能通过说明模块用到了默认未启用的提案，并逐个关闭找出实际需要的那些——这样「用了不支持的扩展」
//! 与「模块损坏」可以区分开。

use serde::Serialize;
use wasmparser::{
    BinaryReaderError, FuncValidatorAllocations, Parser, ValidPayload, Validator, WasmFeatures,
};

use crate::model::Problem;

/// 超过这个大小不再逐个尝试提案（每次尝试都要完整校验一遍）
const DETECT_LIMIT: usize = 32 * 1024 * 1024;
/// 每校验这么多个函数检查一次取消
const CANCEL_EVERY: usize = 256;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Validation {
    /// valid / invalid / needs_features
    pub status: &'static str,
    pub error: Option<Problem>,
    /// 模块需要、但默认特性集没有启用的提案
    pub required: Vec<String>,
    /// 校验使用的特性集（不含 component model 相关）
    pub features: Vec<String>,
    /// 所需提案过多或文件过大时没有逐个识别
    pub detected: bool,
}

pub enum Outcome {
    Valid,
    Invalid(BinaryReaderError),
    Cancelled,
}

/// 特性名称：`MUTABLE_GLOBAL` → `mutable-global`；跳过 component model 相关
pub fn feature_names(features: WasmFeatures) -> Vec<String> {
    features
        .iter_names()
        .filter(|(name, _)| !name.starts_with("CM_") && *name != "COMPONENT_MODEL")
        .map(|(name, _)| name.to_ascii_lowercase().replace('_', "-"))
        .collect()
}

/// 按给定特性集完整校验一遍
pub fn check(bytes: &[u8], features: WasmFeatures, cancelled: &dyn Fn() -> bool) -> Outcome {
    let mut validator = Validator::new_with_features(features);
    let mut allocs = FuncValidatorAllocations::default();
    let mut funcs = 0usize;
    for payload in Parser::new(0).parse_all(bytes) {
        let payload = match payload {
            Ok(p) => p,
            Err(err) => return Outcome::Invalid(err),
        };
        match validator.payload(&payload) {
            Ok(ValidPayload::Func(to_validate, body)) => {
                let mut func = to_validate.into_validator(std::mem::take(&mut allocs));
                if let Err(err) = func.validate(&body) {
                    return Outcome::Invalid(err);
                }
                allocs = func.into_allocations();
                funcs += 1;
                if funcs.is_multiple_of(CANCEL_EVERY) && cancelled() {
                    return Outcome::Cancelled;
                }
            }
            Ok(ValidPayload::End(_)) => break,
            Ok(_) => {}
            Err(err) => return Outcome::Invalid(err),
        }
    }
    Outcome::Valid
}

/// 校验模块；返回 None 表示被取消
pub fn validate(bytes: &[u8], cancelled: &dyn Fn() -> bool) -> Option<Validation> {
    let default = WasmFeatures::default();
    let error = match check(bytes, default, cancelled) {
        Outcome::Valid => {
            return Some(Validation {
                status: "valid",
                error: None,
                required: Vec::new(),
                features: feature_names(default),
                detected: true,
            });
        }
        Outcome::Cancelled => return None,
        Outcome::Invalid(err) => Problem {
            code: "wasm.invalid".into(),
            message: err.message().to_owned(),
            offset: Some(err.offset()),
        },
    };
    let invalid = Validation {
        status: "invalid",
        error: Some(error.clone()),
        required: Vec::new(),
        features: feature_names(default),
        detected: true,
    };
    let all = WasmFeatures::all();
    match check(bytes, all, cancelled) {
        Outcome::Valid => {}
        Outcome::Cancelled => return None,
        Outcome::Invalid(_) => return Some(invalid),
    }
    if bytes.len() > DETECT_LIMIT {
        return Some(Validation {
            status: "needs_features",
            required: Vec::new(),
            detected: false,
            ..invalid
        });
    }
    // 从全部提案开始，逐个去掉不影响通过的提案，剩下的就是需要的
    let mut current = all;
    for (_, flag) in all.difference(default).iter_names() {
        let candidate = current.difference(flag);
        match check(bytes, candidate, cancelled) {
            Outcome::Valid => current = candidate,
            Outcome::Cancelled => return None,
            Outcome::Invalid(_) => {}
        }
    }
    Some(Validation {
        status: "needs_features",
        required: feature_names(current.difference(default)),
        ..invalid
    })
}

#[cfg(test)]
#[path = "validate_test.rs"]
mod tests;
