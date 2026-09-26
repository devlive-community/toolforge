//! 模型会话：ONNX 只解析一次，按输入形状编译并缓存执行计划
//! （检测按图片尺寸、识别按宽度档位），避免每张图都重新优化模型。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tf_plugin_api::{PluginError, PluginResult};
use tract_onnx::prelude::*;

pub type Plan = Arc<TypedRunnableModel>;

/// 检测计划按尺寸缓存的上限
const DETECT_PLANS: usize = 4;

fn invalid(err: impl std::fmt::Display) -> PluginError {
    PluginError::new("ocr.model_invalid").with("detail", err.to_string())
}

fn compile(model: &InferenceModel, shape: [usize; 4]) -> PluginResult<Plan> {
    model
        .clone()
        .with_input_fact(0, f32::fact(shape).into())
        .map_err(invalid)?
        .into_optimized()
        .map_err(invalid)?
        .into_runnable()
        .map_err(invalid)
}

pub struct Engine {
    paths: (PathBuf, PathBuf),
    detector: InferenceModel,
    recognizer: InferenceModel,
    detect_plans: Mutex<Vec<((u32, u32), Plan)>>,
    recognize_plans: Mutex<HashMap<u32, Plan>>,
}

impl Engine {
    fn load(detector: &Path, recognizer: &Path) -> PluginResult<Self> {
        Ok(Self {
            paths: (detector.to_owned(), recognizer.to_owned()),
            detector: tract_onnx::onnx()
                .model_for_path(detector)
                .map_err(invalid)?,
            recognizer: tract_onnx::onnx()
                .model_for_path(recognizer)
                .map_err(invalid)?,
            detect_plans: Mutex::new(Vec::new()),
            recognize_plans: Mutex::new(HashMap::new()),
        })
    }

    pub fn detector(&self, width: u32, height: u32) -> PluginResult<Plan> {
        let mut plans = self.detect_plans.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, plan)) = plans.iter().find(|(size, _)| *size == (width, height)) {
            return Ok(plan.clone());
        }
        let plan = compile(&self.detector, [1, 3, height as usize, width as usize])?;
        plans.push(((width, height), plan.clone()));
        if plans.len() > DETECT_PLANS {
            plans.remove(0);
        }
        Ok(plan)
    }

    pub fn recognizer(&self, width: u32) -> PluginResult<Plan> {
        let mut plans = self
            .recognize_plans
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(plan) = plans.get(&width) {
            return Ok(plan.clone());
        }
        let height = crate::recognize::HEIGHT as usize;
        let plan = compile(&self.recognizer, [1, 3, height, width as usize])?;
        plans.insert(width, plan.clone());
        Ok(plan)
    }
}

#[derive(Default)]
pub struct Engines {
    current: Mutex<Option<Arc<Engine>>>,
}

impl Engines {
    /// 返回引擎以及是否为新加载
    pub fn get(&self, detector: &Path, recognizer: &Path) -> PluginResult<(Arc<Engine>, bool)> {
        let mut current = self.current.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(engine) = current.as_ref()
            && engine.paths.0 == detector
            && engine.paths.1 == recognizer
        {
            return Ok((engine.clone(), false));
        }
        let engine = Arc::new(Engine::load(detector, recognizer)?);
        *current = Some(engine.clone());
        Ok((engine, true))
    }
}

/// 运行模型并取出第一个输出的数据与形状
pub fn run(plan: &Plan, input: Tensor) -> PluginResult<(Vec<f32>, Vec<usize>)> {
    let outputs = plan
        .run(tvec!(input.into()))
        .map_err(|e| PluginError::new("ocr.inference_failed").with("detail", e.to_string()))?;
    let tensor = outputs[0].clone().into_tensor();
    let view = tensor
        .to_plain_array_view::<f32>()
        .map_err(|e| PluginError::new("ocr.inference_failed").with("detail", e.to_string()))?;
    Ok((view.iter().copied().collect(), view.shape().to_vec()))
}
