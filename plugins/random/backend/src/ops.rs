//! 各种随机操作：随机数、抽签、打乱、分组、骰子与硬币。

use rand::Rng;
use rand::seq::SliceRandom;
use rand::seq::index::sample;
use rand_chacha::ChaCha20Rng;
use serde::Serialize;
use tf_plugin_api::{PluginError, PluginResult};

/// 单次最多生成的个数
pub const MAX_COUNT: usize = 10_000;
const MAX_ITEMS: usize = 100_000;

fn too_many(max: usize) -> PluginError {
    PluginError::new("random.too_many").with("max", max)
}

/// 生成 count 个 [min, max] 中的整数；unique 时不重复
pub fn numbers(
    rng: &mut ChaCha20Rng,
    min: i64,
    max: i64,
    count: usize,
    unique: bool,
    sorted: bool,
) -> PluginResult<Vec<i64>> {
    if min > max {
        return Err(PluginError::new("random.invalid_range"));
    }
    if count == 0 {
        return Err(PluginError::new("random.invalid_count"));
    }
    if count > MAX_COUNT {
        return Err(too_many(MAX_COUNT));
    }
    let span = (max as i128 - min as i128 + 1) as u128;
    let mut out: Vec<i64> = if unique {
        if (count as u128) > span {
            return Err(PluginError::new("random.not_enough_numbers")
                .with("available", span.min(u64::MAX as u128) as u64));
        }
        if span <= (MAX_ITEMS as u128) * 10 {
            sample(rng, span as usize, count)
                .into_iter()
                .map(|i| min + i as i64)
                .collect()
        } else {
            // 范围很大时逐个抽取，碰到重复就重抽
            let mut seen = std::collections::HashSet::new();
            let mut out = Vec::with_capacity(count);
            while out.len() < count {
                let v = rng.random_range(min..=max);
                if seen.insert(v) {
                    out.push(v);
                }
            }
            out
        }
    } else {
        (0..count).map(|_| rng.random_range(min..=max)).collect()
    };
    if sorted {
        out.sort_unstable();
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Candidate {
    pub name: String,
    pub weight: u32,
}

/// 每行一个名字；“名字*3”表示 3 份机会（只在 weighted 时生效）
pub fn parse_items(text: &str, weighted: bool) -> PluginResult<Vec<Candidate>> {
    let mut out = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let (name, weight) = match line.rsplit_once('*') {
            Some((name, w)) if weighted && !name.trim().is_empty() => match w.trim().parse::<u32>()
            {
                Ok(w) if (1..=1000).contains(&w) => (name.trim(), w),
                _ => return Err(PluginError::new("random.invalid_weight").with("line", line)),
            },
            _ => (line, 1),
        };
        out.push(Candidate {
            name: name.to_owned(),
            weight,
        });
    }
    if out.is_empty() {
        return Err(PluginError::new("random.no_items"));
    }
    if out.len() > MAX_ITEMS {
        return Err(too_many(MAX_ITEMS));
    }
    Ok(out)
}

/// 不放回地抽取 count 个；按权重时权重越大越容易被抽中
pub fn draw(rng: &mut ChaCha20Rng, items: &[Candidate], count: usize) -> PluginResult<Vec<String>> {
    if count == 0 {
        return Err(PluginError::new("random.invalid_count"));
    }
    if count > items.len() {
        return Err(PluginError::new("random.not_enough_items").with("available", items.len()));
    }
    let mut pool: Vec<&Candidate> = items.iter().collect();
    let mut winners = Vec::with_capacity(count);
    for _ in 0..count {
        let total: u64 = pool.iter().map(|c| c.weight as u64).sum();
        let mut pick = rng.random_range(0..total);
        let index = pool
            .iter()
            .position(|c| {
                if pick < c.weight as u64 {
                    true
                } else {
                    pick -= c.weight as u64;
                    false
                }
            })
            .unwrap_or(pool.len() - 1);
        winners.push(pool.remove(index).name.clone());
    }
    Ok(winners)
}

pub fn shuffle(rng: &mut ChaCha20Rng, items: &[Candidate]) -> Vec<String> {
    let mut names: Vec<String> = items.iter().map(|c| c.name.clone()).collect();
    names.shuffle(rng);
    names
}

/// 分组：groups 为组数，或 size 为每组人数；先打乱再依次轮流分入，组间人数最多差 1
pub fn groups(
    rng: &mut ChaCha20Rng,
    items: &[Candidate],
    count: Option<usize>,
    size: Option<usize>,
    shuffled: bool,
) -> PluginResult<Vec<Vec<String>>> {
    let n = items.len();
    let groups = match (count, size) {
        (Some(g), _) if g >= 1 => g,
        (None, Some(s)) if s >= 1 => n.div_ceil(s),
        _ => return Err(PluginError::new("random.invalid_count")),
    };
    if groups > n {
        return Err(PluginError::new("random.not_enough_items").with("available", n));
    }
    let names = if shuffled {
        shuffle(rng, items)
    } else {
        items.iter().map(|c| c.name.clone()).collect()
    };
    let mut out = vec![Vec::new(); groups];
    // 按组大小顺序切分，前面的组多 1 人，保持原顺序时也直观
    let base = n / groups;
    let extra = n % groups;
    let mut iter = names.into_iter();
    for (i, group) in out.iter_mut().enumerate() {
        let take = base + usize::from(i < extra);
        group.extend(iter.by_ref().take(take));
    }
    Ok(out)
}

pub fn dice(rng: &mut ChaCha20Rng, count: usize, sides: u32) -> PluginResult<Vec<u32>> {
    if count == 0 || count > 1000 {
        return Err(too_many(1000));
    }
    if !(2..=1000).contains(&sides) {
        return Err(PluginError::new("random.invalid_sides"));
    }
    Ok((0..count).map(|_| rng.random_range(1..=sides)).collect())
}

#[cfg(test)]
#[path = "ops_test.rs"]
mod tests;
