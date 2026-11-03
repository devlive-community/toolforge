//! 宾果活动卡插件后端：从词条池生成 3×3 / 5×5 卡片（唯一布局校验）、
//! 不放回抽取叫号、撤销与连线判定（横/竖/两条主对角线，中央免叫格自动命中）。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct BingoCards {
    manifest: Manifest,
}

impl Default for BingoCards {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for BingoCards {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "generate" => to_value(generate(parse_args(args)?)?),
            "draw" => to_value(draw(parse_args(args)?)?),
            "check" => to_value(check(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(BingoCards);

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    pub fn draw(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.draw() % n as u64) as usize
    }
}

fn shuffled<T>(items: &mut [T], rng: &mut Rng) {
    for k in (1..items.len()).rev() {
        items.swap(k, rng.below(k + 1));
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateArgs {
    /// 词条池（去重后使用）
    pub items: Vec<String>,
    /// 3 或 5
    pub size: usize,
    /// 卡片数量（1–50）
    pub cards: usize,
    /// 是否中央免叫格（5×5 才有意义；3×3 传 true 时忽略）
    #[serde(default)]
    pub free_center: bool,
    pub seed: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedCard {
    /// 卡号（1 起）
    pub number: u64,
    /// 行优先词条；free 格为 "FREE"
    pub cells: Vec<String>,
    pub free_index: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateOutput {
    pub size: usize,
    pub cards: Vec<GeneratedCard>,
    /// 每张卡用到的词条集合（去重计数）
    pub items_per_card: usize,
}

pub fn generate(args: GenerateArgs) -> PluginResult<GenerateOutput> {
    let size = args.size;
    if size != 3 && size != 5 {
        return Err(PluginError::new("bingo.bad_size").with("size", size as u64));
    }
    if args.cards == 0 || args.cards > 50 {
        return Err(PluginError::new("bingo.bad_card_count").with("cards", args.cards as u64));
    }
    let free_center = size == 5 && args.free_center;
    let slots = size * size - usize::from(free_center);
    // 词条去重
    let mut pool: Vec<String> = Vec::new();
    for item in &args.items {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            return Err(PluginError::new("bingo.empty_item"));
        }
        if !pool.iter().any(|p| p == trimmed) {
            pool.push(trimmed.to_owned());
        }
    }
    if pool.len() < slots {
        return Err(PluginError::new("bingo.not_enough_items")
            .with("have", pool.len() as u64)
            .with("need", slots as u64));
    }
    if pool.len() > 200 {
        return Err(PluginError::new("bingo.too_many_items").with("max", 200));
    }
    let mut rng = Rng::new(args.seed);
    let mut seen: Vec<Vec<String>> = Vec::new();
    let mut cards = Vec::new();
    // 生成尝试上限：布局完全相同的卡不算新卡
    for number in 1..=args.cards as u64 {
        let mut attempt = 0;
        loop {
            attempt += 1;
            if attempt > 100 {
                return Err(PluginError::new("bingo.layout_exhausted").with("cards", number));
            }
            let mut bag = pool.clone();
            shuffled(&mut bag, &mut rng);
            let cells: Vec<String> = bag.into_iter().take(slots).collect();
            if seen.contains(&cells) {
                continue;
            }
            seen.push(cells.clone());
            // 按行优先展开，free 格插入中点
            let mut full: Vec<String> = Vec::with_capacity(size * size);
            let free_index = if free_center {
                Some(size * size / 2)
            } else {
                None
            };
            let mut iter = cells.into_iter();
            for index in 0..size * size {
                if free_index == Some(index) {
                    full.push("FREE".into());
                } else {
                    full.push(iter.next().unwrap_or_default());
                }
            }
            cards.push(GeneratedCard {
                number,
                cells: full,
                free_index,
            });
            break;
        }
    }
    let items_per_card = slots;
    Ok(GenerateOutput {
        size,
        cards,
        items_per_card,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DrawArgs {
    /// 尚未叫出的词条
    pub remaining: Vec<String>,
    /// 抽取序号（用于种子派生，保证同一历史可复现）
    pub seed: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DrawOutput {
    pub item: String,
    pub remaining: Vec<String>,
}

/// 不放回抽取一个词条
pub fn draw(args: DrawArgs) -> PluginResult<DrawOutput> {
    if args.remaining.is_empty() {
        return Err(PluginError::new("bingo.pool_exhausted"));
    }
    let mut rng = Rng::new(args.seed);
    let index = rng.below(args.remaining.len());
    let item = args.remaining[index].clone();
    let mut remaining = args.remaining;
    remaining.remove(index);
    Ok(DrawOutput { item, remaining })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckArgs {
    pub size: usize,
    /// 卡面（行优先，free 格为 "FREE"）
    pub cells: Vec<String>,
    #[serde(default)]
    pub free_center: bool,
    /// 已叫出的词条集合
    pub called: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    /// kind：row / column / diagonal
    pub kind: &'static str,
    pub index: usize,
    /// 组成连线的格子下标
    pub cells: Vec<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckOutput {
    pub has_line: bool,
    pub lines: Vec<Line>,
    /// 未叫却被勾选/要求核对的词条（核对用）
    pub uncalled_used: Vec<String>,
}

pub fn check(args: CheckArgs) -> PluginResult<CheckOutput> {
    let size = args.size;
    if size != 3 && size != 5 {
        return Err(PluginError::new("bingo.bad_size").with("size", size as u64));
    }
    if args.cells.len() != size * size {
        return Err(PluginError::new("bingo.invalid_grid"));
    }
    let free_center = size == 5 && args.free_center;
    let free_index = if free_center {
        Some(size * size / 2)
    } else {
        None
    };
    let is_hit = |index: usize| -> bool {
        if free_index == Some(index) {
            return true;
        }
        let Some(cell) = args.cells.get(index) else {
            return false;
        };
        args.called.iter().any(|c| c == cell)
    };
    let mut lines = Vec::new();
    for row in 0..size {
        let cells: Vec<usize> = (0..size).map(|c| row * size + c).collect();
        if cells.iter().all(|&i| is_hit(i)) {
            lines.push(Line {
                kind: "row",
                index: row,
                cells,
            });
        }
    }
    for col in 0..size {
        let cells: Vec<usize> = (0..size).map(|r| r * size + col).collect();
        if cells.iter().all(|&i| is_hit(i)) {
            lines.push(Line {
                kind: "column",
                index: col,
                cells,
            });
        }
    }
    let diag1: Vec<usize> = (0..size).map(|i| i * size + i).collect();
    if diag1.iter().all(|&i| is_hit(i)) {
        lines.push(Line {
            kind: "diagonal",
            index: 0,
            cells: diag1,
        });
    }
    let diag2: Vec<usize> = (0..size).map(|i| i * size + (size - 1 - i)).collect();
    if diag2.iter().all(|&i| is_hit(i)) {
        lines.push(Line {
            kind: "diagonal",
            index: 1,
            cells: diag2,
        });
    }
    // 核对：卡面里被叫池外命中的项（free 除外）
    let uncalled_used: Vec<String> = args
        .cells
        .iter()
        .enumerate()
        .filter(|(i, cell)| free_index != Some(*i) && !args.called.iter().any(|c| c == *cell))
        .map(|(_, cell)| cell.clone())
        .collect();
    Ok(CheckOutput {
        has_line: !lines.is_empty(),
        lines,
        uncalled_used,
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
