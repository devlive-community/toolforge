//! 照片拼图：切片参数生成、吸附判定与完成校验；游玩状态与成绩由宿主 SQLite 保存。

mod edges;
mod prepare;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, parse_args, to_value,
    unknown_function,
};

use crate::edges::EdgeGrid;
use crate::prepare::ImageInput;

const MANIFEST: &str = include_str!("../../manifest.json");
/// 允许的片数规格
pub const GRIDS: &[(u32, u32)] = &[(3, 3), (4, 4), (6, 6)];
/// 存档数量上限（LRU 之外淘汰，由前端维护）
pub const SAVE_LIMIT: usize = 3;

fn errp(code: &str, key: &str, value: impl Into<serde_json::Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

pub struct Jigsaw {
    manifest: Manifest,
}

impl Default for Jigsaw {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

/// 选择照片并压成受限工作图（非任务）
fn prepare_fn(args: Value) -> PluginResult<Value> {
    let input: ImageInput = parse_args(args)?;
    to_value(prepare::prepare(&input)?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewGameArgs {
    /// prepare 输出的工作图
    work_data: String,
    /// 取景范围（工作图像素坐标）
    crop_x: u32,
    crop_y: u32,
    crop_w: u32,
    crop_h: u32,
    cols: u32,
    rows: u32,
    /// 不传则由内容推导的确定性种子
    #[serde(default)]
    seed: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GameSetup {
    /// 裁剪后的游戏图（JPEG Data URI）
    game_data: String,
    width: u32,
    height: u32,
    cols: u32,
    rows: u32,
    seed: u64,
    cell_w: f64,
    cell_h: f64,
    #[serde(flatten)]
    edges: EdgeGrid,
    /// 未放置拼片的初始散落位置（相对游戏图宽高，可为负）
    tray: Vec<[f64; 2]>,
}

fn xorshift(seed: u64) -> impl FnMut() -> f64 {
    let mut state = seed.max(1);
    move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// 开局：裁剪取景范围、生成边缘参数与散落位置（任务，可取消）
fn new_game(args: NewGameArgs, ctx: &dyn TaskContext) -> PluginResult<GameSetup> {
    let (cols, rows) = (args.cols, args.rows);
    if !GRIDS.contains(&(cols, rows)) {
        return Err(errp("jigsaw.bad_grid", "value", format!("{cols}x{rows}")));
    }
    if ctx.is_cancelled() {
        return Err(tf_plugin_api::cancelled());
    }
    let image = prepare::decode_data_uri(&args.work_data)?;
    ctx.progress(40, 100);
    if ctx.is_cancelled() {
        return Err(tf_plugin_api::cancelled());
    }
    let crop_x = args.crop_x.min(image.width().saturating_sub(1));
    let crop_y = args.crop_y.min(image.height().saturating_sub(1));
    let crop_w = args.crop_w.min(image.width() - crop_x);
    let crop_h = args.crop_h.min(image.height() - crop_y);
    if crop_w < cols || crop_h < rows || crop_w < 90 || crop_h < 90 {
        return Err(errp(
            "jigsaw.crop_too_small",
            "value",
            format!("{crop_w}x{crop_h}"),
        ));
    }
    let cropped = image::imageops::crop_imm(&image, crop_x, crop_y, crop_w, crop_h).to_image();
    ctx.progress(70, 100);
    if ctx.is_cancelled() {
        return Err(tf_plugin_api::cancelled());
    }
    let bytes = prepare::work_image(
        &image::DynamicImage::ImageRgba8(cropped),
        prepare::WORK_LONG_EDGE,
    )?;
    let sized = prepare::decode(&bytes)?;
    let (width, height) = (sized.width(), sized.height());
    let seed = if args.seed == 0 {
        // 从裁剪内容推导确定性种子，重开同参数可复现
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        use std::hash::{Hash, Hasher};
        (width, height, cols, rows, &bytes).hash(&mut hasher);
        hasher.finish()
    } else {
        args.seed
    };
    let edges = edges::generate(cols, rows, seed);
    let mut scatter = xorshift(seed.rotate_left(17));
    let tray = (0..cols * rows)
        .map(|_| {
            let side = (scatter() * 4.0) as u32 % 4;
            let t = scatter() * 1.2 - 0.1;
            let band = 0.05 + scatter() * 0.3;
            match side {
                0 => [t, -band],
                1 => [t, 1.0 + band],
                2 => [-band, t],
                _ => [1.0 + band, t],
            }
        })
        .collect();
    ctx.progress(100, 100);
    Ok(GameSetup {
        game_data: prepare::to_data_uri(&bytes),
        width,
        height,
        cols,
        rows,
        seed,
        cell_w: f64::from(width) / f64::from(cols),
        cell_h: f64::from(height) / f64::from(rows),
        edges,
        tray,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlaceArgs {
    row: u32,
    col: u32,
    /// 拖放后拼片格位起点（游戏图坐标）
    x: f64,
    y: f64,
    cell_w: f64,
    cell_h: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaceOut {
    placed: bool,
    x: f64,
    y: f64,
}

/// 吸附判定（非任务）：按拼片身份与目标格位判断，不按画面猜测
fn place_fn(args: PlaceArgs) -> PlaceOut {
    let (placed, x, y) = edges::judge(args.row, args.col, args.x, args.y, args.cell_w, args.cell_h);
    PlaceOut { placed, x, y }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VerifyArgs {
    cols: u32,
    rows: u32,
    cell_w: f64,
    cell_h: f64,
    /// 全部拼片：[row, col, x, y, placed]
    placements: Vec<[f64; 5]>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct VerifyOut {
    placed: u32,
    total: u32,
    /// 每片恰好一次且已放片全部位于精确格位
    valid: bool,
}

/// 完成校验（非任务）：身份唯一 + 已放片必须精确在格位，杜绝仅“看起来填满”
fn verify_fn(args: VerifyArgs) -> PluginResult<VerifyOut> {
    let total = args.cols * args.rows;
    if args.placements.len() != total as usize {
        return Err(errp("jigsaw.piece_count", "value", args.placements.len()));
    }
    let mut seen = std::collections::HashSet::new();
    let mut placed = 0u32;
    let mut valid = true;
    for slot in &args.placements {
        let (row, col, x, y, is_placed) = (slot[0], slot[1], slot[2], slot[3], slot[4] >= 0.5);
        if row.fract() != 0.0
            || col.fract() != 0.0
            || row < 0.0
            || col < 0.0
            || row >= f64::from(args.rows)
            || col >= f64::from(args.cols)
            || !seen.insert((row as u32, col as u32))
        {
            return Err(errp("jigsaw.bad_identity", "value", format!("{row},{col}")));
        }
        if !is_placed {
            continue;
        }
        placed += 1;
        let target_x = col * args.cell_w;
        let target_y = row * args.cell_h;
        if (x - target_x).abs() > 0.5 || (y - target_y).abs() > 0.5 {
            valid = false;
        }
    }
    if placed != total {
        valid = false;
    }
    Ok(VerifyOut {
        placed,
        total,
        valid,
    })
}

impl ToolPlugin for Jigsaw {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "prepare" => prepare_fn(args),
            "place" => to_value(place_fn(parse_args(args)?)),
            "verify" => to_value(verify_fn(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "newgame" => to_value(new_game(parse_args(args)?, ctx)?),
            other => self.call(other, args),
        }
    }
}

tf_plugin_guest::export_plugin!(Jigsaw);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
