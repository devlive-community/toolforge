use std::path::Path;
use std::sync::{Arc, Mutex};

use image::imageops::FilterType;
use image::{GrayImage, Luma, RgbImage};
use serde::Deserialize;
use tf_plugin_api::{PluginError, PluginResult};
use tract_onnx::prelude::*;

/// 可选的分割模型（id 与 manifest 中的资源 id 一致）
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
pub enum Model {
    /// U²-Net 轻量版：约 4.4 MB，速度快
    #[default]
    #[serde(rename = "u2netp")]
    U2netp,
    /// IS-Net 通用版：约 170 MB，边缘更精细
    #[serde(rename = "isnet-general-use")]
    Isnet,
}

impl Model {
    pub fn resource_id(self) -> &'static str {
        match self {
            Self::U2netp => "u2netp",
            Self::Isnet => "isnet-general-use",
        }
    }

    /// 模型输入边长
    pub fn input_size(self) -> u32 {
        match self {
            Self::U2netp => 320,
            Self::Isnet => 1024,
        }
    }

    /// 与训练时一致的归一化参数（均值、标准差）
    pub fn normalization(self) -> ([f32; 3], [f32; 3]) {
        match self {
            Self::U2netp => ([0.485, 0.456, 0.406], [0.229, 0.224, 0.225]),
            Self::Isnet => ([0.5; 3], [1.0; 3]),
        }
    }
}

pub type SharedPlan = Arc<TypedRunnableModel>;

/// 已加载的模型缓存；切换模型时替换，避免每次任务都重新解析与优化
#[derive(Default)]
pub struct Sessions {
    current: Mutex<Option<(Model, SharedPlan)>>,
}

fn invalid(err: impl std::fmt::Display) -> PluginError {
    PluginError::new("bg.model_invalid").with("detail", err.to_string())
}

impl Sessions {
    /// 返回模型会话以及是否为新加载
    pub fn get(&self, model: Model, path: &Path) -> PluginResult<(SharedPlan, bool)> {
        let mut current = self.current.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((loaded, plan)) = current.as_ref()
            && *loaded == model
        {
            return Ok((plan.clone(), false));
        }
        let size = model.input_size() as usize;
        let plan = tract_onnx::onnx()
            .model_for_path(path)
            .map_err(invalid)?
            .with_input_fact(0, f32::fact([1, 3, size, size]).into())
            .map_err(invalid)?
            .into_optimized()
            .map_err(invalid)?
            .into_runnable()
            .map_err(invalid)?;
        *current = Some((model, plan.clone()));
        Ok((plan, true))
    }
}

/// 按模型要求缩放并归一化为 NCHW 张量
pub fn preprocess(image: &RgbImage, model: Model) -> Tensor {
    let size = model.input_size();
    let resized = image::imageops::resize(image, size, size, FilterType::Triangle);
    let (mean, std) = model.normalization();
    // 与 rembg 一致：先除以图像最大像素值
    let max = resized
        .pixels()
        .flat_map(|p| p.0)
        .max()
        .unwrap_or(255)
        .max(1) as f32;
    let size = size as usize;
    tract_ndarray::Array4::from_shape_fn((1, 3, size, size), |(_, c, y, x)| {
        (resized.get_pixel(x as u32, y as u32)[c] as f32 / max - mean[c]) / std[c]
    })
    .into()
}

/// 把模型输出（1×1×S×S）按最小-最大值归一化为 0-255 的蒙版
pub fn mask_from_output(output: &Tensor, size: u32) -> PluginResult<GrayImage> {
    let view = output
        .to_plain_array_view::<f32>()
        .map_err(|e| PluginError::new("bg.inference_failed").with("detail", e.to_string()))?;
    let shape = view.shape().to_vec();
    if shape.len() != 4 || shape[2] != size as usize || shape[3] != size as usize {
        return Err(PluginError::new("bg.inference_failed")
            .with("detail", format!("unexpected output shape {shape:?}")));
    }
    let (lo, hi) = view
        .iter()
        .fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
    let range = (hi - lo).max(f32::EPSILON);
    Ok(GrayImage::from_fn(size, size, |x, y| {
        let v = (view[[0, 0, y as usize, x as usize]] - lo) / range;
        Luma([(v.clamp(0.0, 1.0) * 255.0).round() as u8])
    }))
}

/// 运行分割模型，返回与原图同尺寸的蒙版
pub fn predict(plan: &SharedPlan, image: &RgbImage, model: Model) -> PluginResult<GrayImage> {
    let input = preprocess(image, model);
    let outputs = plan
        .run(tvec!(input.into()))
        .map_err(|e| PluginError::new("bg.inference_failed").with("detail", e.to_string()))?;
    let first = outputs
        .first()
        .ok_or_else(|| PluginError::new("bg.inference_failed").with("detail", "no output"))?;
    let mask = mask_from_output(&first.clone().into_tensor(), model.input_size())?;
    Ok(image::imageops::resize(
        &mask,
        image.width(),
        image.height(),
        FilterType::Triangle,
    ))
}

#[cfg(test)]
#[path = "model_test.rs"]
mod tests;
