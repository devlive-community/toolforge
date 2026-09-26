//! 方形化树图布局（Bruls 等人的 squarified 算法）：让矩形尽量接近正方形，便于比较大小。

use serde::Serialize;

use crate::category::Category;
use crate::tree::{NodeId, Tree};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// 把按降序排列的数值铺满矩形；返回与输入一一对应的矩形
pub fn squarify(values: &[f64], rect: Rect) -> Vec<Rect> {
    let total: f64 = values.iter().sum();
    if values.is_empty() || total <= 0.0 || rect.w <= 0.0 || rect.h <= 0.0 {
        return vec![
            Rect {
                w: 0.0,
                h: 0.0,
                ..rect
            };
            values.len()
        ];
    }
    let scale = rect.w * rect.h / total;
    let areas: Vec<f64> = values.iter().map(|v| v * scale).collect();
    let mut out = Vec::with_capacity(areas.len());
    let mut free = rect;
    let mut start = 0;
    while start < areas.len() {
        let side = free.w.min(free.h);
        // 逐个加入当前行，直到最差长宽比变差
        let mut end = start + 1;
        let mut best = worst(&areas[start..end], side);
        while end < areas.len() {
            let next = worst(&areas[start..=end], side);
            if next > best {
                break;
            }
            best = next;
            end += 1;
        }
        let row = &areas[start..end];
        let sum: f64 = row.iter().sum();
        if free.w >= free.h {
            // 竖着放一列在左侧
            let width = if free.h > 0.0 { sum / free.h } else { 0.0 };
            let mut y = free.y;
            for area in row {
                let h = if width > 0.0 { area / width } else { 0.0 };
                out.push(Rect {
                    x: free.x,
                    y,
                    w: width,
                    h,
                });
                y += h;
            }
            free = Rect {
                x: free.x + width,
                w: (free.w - width).max(0.0),
                ..free
            };
        } else {
            let height = if free.w > 0.0 { sum / free.w } else { 0.0 };
            let mut x = free.x;
            for area in row {
                let w = if height > 0.0 { area / height } else { 0.0 };
                out.push(Rect {
                    x,
                    y: free.y,
                    w,
                    h: height,
                });
                x += w;
            }
            free = Rect {
                y: free.y + height,
                h: (free.h - height).max(0.0),
                ..free
            };
        }
        start = end;
    }
    out
}

/// 一行矩形中最差的长宽比
fn worst(row: &[f64], side: f64) -> f64 {
    let sum: f64 = row.iter().sum();
    let (min, max) = row
        .iter()
        .fold((f64::MAX, 0.0f64), |(lo, hi), a| (lo.min(*a), hi.max(*a)));
    if sum <= 0.0 || min <= 0.0 {
        return f64::MAX;
    }
    let side2 = side * side;
    let sum2 = sum * sum;
    (side2 * max / sum2).max(sum2 / (side2 * min))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tile {
    /// 合并的小项没有 id
    pub id: Option<NodeId>,
    pub name: String,
    pub size: u64,
    pub dir: bool,
    pub category: Category,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub depth: u8,
    /// 合并的小项数量
    pub merged: usize,
}

/// 面积小于该值（像素²）的项合并为一块
const MIN_AREA: f64 = 36.0;
const MAX_TILES: usize = 400;
/// 最多绘制的层数（当前文件夹的子项为第 0 层）
const MAX_DEPTH: u8 = 3;
/// 文件夹标题栏高度与内边距
const HEADER: f64 = 18.0;
const PAD: f64 = 2.0;

fn layout(tree: &Tree, id: NodeId, rect: Rect, depth: u8, max_depth: u8, out: &mut Vec<Tile>) {
    let node = &tree.nodes[id as usize];
    let total = node.size.max(1) as f64;
    let area = rect.w * rect.h;
    // 选出足够大的子项，其余合并
    let mut shown: Vec<NodeId> = Vec::new();
    let mut merged = (0u64, 0usize);
    for child in &node.children {
        let size = tree.nodes[*child as usize].size;
        if size == 0 {
            continue;
        }
        if size as f64 / total * area >= MIN_AREA && shown.len() < MAX_TILES {
            shown.push(*child);
        } else {
            merged.0 += size;
            merged.1 += 1;
        }
    }
    let mut values: Vec<f64> = shown
        .iter()
        .map(|c| tree.nodes[*c as usize].size as f64)
        .collect();
    if merged.0 > 0 {
        values.push(merged.0 as f64);
    }
    // 合并项通常最小，放在末尾仍保持降序；若不是则重新排序会打乱对应关系，这里按原顺序即可
    let rects = squarify(&values, rect);
    for (index, r) in rects.into_iter().enumerate() {
        if index < shown.len() {
            let child = shown[index];
            let c = &tree.nodes[child as usize];
            out.push(Tile {
                id: Some(child),
                name: c.name.to_string(),
                size: c.size,
                dir: c.dir,
                category: c.category,
                x: r.x,
                y: r.y,
                w: r.w,
                h: r.h,
                depth,
                merged: 0,
            });
            // 足够大的文件夹再画一层子项
            if c.dir && depth + 1 < max_depth && r.w > 60.0 && r.h > 40.0 {
                let inner = Rect {
                    x: r.x + PAD,
                    y: r.y + HEADER,
                    w: (r.w - PAD * 2.0).max(0.0),
                    h: (r.h - HEADER - PAD).max(0.0),
                };
                layout(tree, child, inner, depth + 1, max_depth, out);
            }
        } else {
            out.push(Tile {
                id: None,
                name: String::new(),
                size: merged.0,
                dir: false,
                category: Category::Other,
                x: r.x,
                y: r.y,
                w: r.w,
                h: r.h,
                depth,
                merged: merged.1,
            });
        }
    }
}

pub fn tiles(tree: &Tree, id: NodeId, width: f64, height: f64) -> Vec<Tile> {
    let mut out = Vec::new();
    layout(
        tree,
        id,
        Rect {
            x: 0.0,
            y: 0.0,
            w: width,
            h: height,
        },
        0,
        MAX_DEPTH,
        &mut out,
    );
    out
}

#[cfg(test)]
#[path = "treemap_test.rs"]
mod tests;
