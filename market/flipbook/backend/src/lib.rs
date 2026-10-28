//! 翻页动画小工坊后端：笔画栅格化、图片导入合成、帧操作与上限校验、GIF / PNG 序列导出。
//! 作品与各帧内容由前端按键分别存进 ToolForge SQLite（不把全部帧塞进一个键），
//! 这里只做无状态计算：每次调用都传入需要的作品与帧内容。

mod export;
mod frame;
mod import;
mod project;
mod stroke;

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{
    Manifest, PluginResult, TaskContext, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

/// 后端分发的全部函数；宿主只放行 manifest 中声明过的函数，测试据此核对两边一致
#[cfg(test)]
const FUNCTIONS: [&str; 7] = [
    "limits", "create", "edit", "collect", "stroke", "import", "export",
];

/// 画笔调色板
const PALETTE: [&str; 10] = [
    "#1f2937", "#ffffff", "#ef4444", "#f97316", "#facc15", "#22c55e", "#06b6d4", "#3b82f6",
    "#8b5cf6", "#ec4899",
];
/// GIF 底色候选
const BACKGROUNDS: [&str; 4] = ["#ffffff", "#000000", "#fef3c7", "#e0f2fe"];

pub struct Flipbook {
    manifest: Manifest,
}

impl Default for Flipbook {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

#[derive(Debug, Deserialize)]
struct CreateArgs {
    id: String,
    name: String,
    width: u32,
    height: u32,
}

#[derive(Debug, Deserialize)]
struct EditArgs {
    project: project::Project,
    op: project::Op,
    /// 当前各帧存档文本长度（空白帧可省略）
    #[serde(default)]
    sizes: BTreeMap<String, u64>,
}

#[derive(Debug, Deserialize)]
struct CollectArgs {
    project: project::Project,
    /// 撤销历史仍引用的帧
    #[serde(default)]
    keep: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct StrokeArgs {
    width: u32,
    height: u32,
    /// 当前帧存档文本（空白帧为空）
    #[serde(default)]
    frame: String,
    stroke: stroke::Stroke,
}

#[derive(Debug, Deserialize)]
struct ImportArgs {
    width: u32,
    height: u32,
    #[serde(default)]
    frame: String,
    path: String,
}

fn frame_out(image: &image::RgbaImage) -> PluginResult<Value> {
    let text = frame::encode(image)?;
    Ok(json!({ "size": text.len(), "frame": text }))
}

fn limits() -> Value {
    json!({
        "sizes": frame::SIZES.iter().map(|(w, h)| json!({ "width": w, "height": h })).collect::<Vec<_>>(),
        "maxFrames": frame::MAX_FRAMES,
        "maxProjectMb": frame::MAX_PROJECT_TEXT / (1024 * 1024),
        "minDelay": project::MIN_DELAY,
        "maxDelay": project::MAX_DELAY,
        "delayStep": project::DELAY_STEP,
        "minBrush": stroke::MIN_SIZE,
        "maxBrush": stroke::MAX_SIZE,
        // 画笔调色板与 GIF 底色候选（颜色值只在后端出现，界面不写死颜色）
        "palette": PALETTE,
        "backgrounds": BACKGROUNDS,
    })
}

fn stroke_call(args: Value) -> PluginResult<Value> {
    let args: StrokeArgs = parse_args(args)?;
    let mut image = frame::decode(&args.frame, args.width, args.height)?;
    stroke::apply(&mut image, &args.stroke)?;
    frame_out(&image)
}

fn import_call(args: Value) -> PluginResult<Value> {
    let args: ImportArgs = parse_args(args)?;
    let mut image = frame::decode(&args.frame, args.width, args.height)?;
    if args.path.is_empty() || args.path.len() > 4096 {
        return Err(frame::error("bad_path"));
    }
    let bytes = tf_plugin_guest::net::read_to_end(&args.path, import::MAX_FILE)?;
    let picture = import::decode(&bytes)?;
    import::place(&mut image, &picture);
    frame_out(&image)
}

/// 宿主写文件（需要 fs:user-selected 权限，路径来自用户的保存/选择对话框）
struct HostSink;

impl export::Sink for HostSink {
    fn write(&mut self, path: &str, data: &[u8]) -> PluginResult<()> {
        tf_plugin_guest::net::write_file(path, data)
    }
}

impl ToolPlugin for Flipbook {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "limits" => Ok(limits()),
            "create" => {
                let args: CreateArgs = parse_args(args)?;
                to_value(project::create(
                    &args.id,
                    &args.name,
                    args.width,
                    args.height,
                )?)
            }
            "edit" => {
                let args: EditArgs = parse_args(args)?;
                to_value(project::apply(args.project, args.op, &args.sizes)?)
            }
            "collect" => {
                let args: CollectArgs = parse_args(args)?;
                let (project, remove) = project::collect(args.project, &args.keep)?;
                Ok(json!({ "project": project, "remove": remove }))
            }
            "stroke" => stroke_call(args),
            "import" => import_call(args),
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "export" => {
                let req: export::Request = parse_args(args)?;
                to_value(export::run(&req, ctx, &mut HostSink)?)
            }
            other => self.call(other, args),
        }
    }
}

tf_plugin_guest::export_plugin!(Flipbook);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
