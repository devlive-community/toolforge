use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{LogLevel, PluginResult, TaskContext};

use crate::model::{ALGORITHM, Compiled, MAX_CASES, MAX_COMBINATIONS, error, scalar};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Limits {
    pub max_cases: usize,
    pub budget_ms: u64,
    pub max_combinations: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_cases: 500,
            budget_ms: 5_000,
            max_combinations: MAX_COMBINATIONS,
        }
    }
}

impl Limits {
    pub fn validate(&self) -> PluginResult<()> {
        if !(1..=MAX_CASES).contains(&self.max_cases)
            || !(1..=5_000).contains(&self.budget_ms)
            || !(1..=MAX_COMBINATIONS).contains(&self.max_combinations)
        {
            return Err(error("limits"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Pair {
    pub left: usize,
    pub left_value: usize,
    pub right: usize,
    pub right_value: usize,
    pub state: &'static str,
    /// One-based case identifiers, recomputed by the independent verifier.
    pub cases: Vec<usize>,
}

pub struct Report {
    pub compiled: Compiled,
    pub seed: String,
    pub limits: Limits,
    pub cases: Vec<Vec<usize>>,
    pub pairs: Vec<Pair>,
    pub enumerated: usize,
    pub valid_combinations: usize,
    pub feasibility_complete: bool,
    pub status: &'static str,
    pub reason: Option<&'static str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayFactor {
    pub name: String,
    pub values: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub handle: String,
    pub algorithm: &'static str,
    pub seed: String,
    pub status: &'static str,
    pub reason: Option<&'static str>,
    pub factors: Vec<DisplayFactor>,
    pub model_text: String,
    pub combinations: usize,
    pub enumerated: usize,
    pub valid_combinations: usize,
    pub feasibility_complete: bool,
    pub case_count: usize,
    pub possible_pairs: usize,
    pub feasible_pairs: usize,
    pub covered: usize,
    pub uncovered: usize,
    pub infeasible: usize,
    pub unknown: usize,
    pub coverage_percent: Option<f64>,
}

impl Report {
    pub fn summary(&self, handle: String) -> Summary {
        let count = |state| self.pairs.iter().filter(|p| p.state == state).count();
        let covered = count("covered");
        let feasible = covered + count("uncovered");
        Summary {
            handle,
            algorithm: ALGORITHM,
            seed: self.seed.clone(),
            status: self.status,
            reason: self.reason,
            factors: self
                .compiled
                .model
                .factors
                .iter()
                .map(|f| DisplayFactor {
                    name: f.name.clone(),
                    values: f.values.iter().map(Value::to_string).collect(),
                })
                .collect(),
            model_text: self.compiled.model_text(),
            combinations: self.compiled.combinations,
            enumerated: self.enumerated,
            valid_combinations: self.valid_combinations,
            feasibility_complete: self.feasibility_complete,
            case_count: self.cases.len(),
            possible_pairs: self.pairs.len(),
            feasible_pairs: feasible,
            covered,
            uncovered: count("uncovered"),
            infeasible: count("infeasible"),
            unknown: count("unknown"),
            coverage_percent: (self.feasibility_complete
                && feasible > 0
                && (covered < feasible || self.status == "complete"))
                .then(|| covered as f64 * 100.0 / feasible as f64),
        }
    }

    pub fn case_maps(&self) -> Vec<BTreeMap<String, Value>> {
        self.cases
            .iter()
            .map(|row| {
                self.compiled
                    .model
                    .factors
                    .iter()
                    .enumerate()
                    .map(|(f, factor)| (factor.name.clone(), factor.values[row[f]].clone()))
                    .collect()
            })
            .collect()
    }
}

struct Catalog {
    pairs: Vec<Pair>,
    offsets: Vec<Vec<usize>>,
}

impl Catalog {
    fn new(compiled: &Compiled) -> Self {
        let factors = &compiled.model.factors;
        let mut pairs = Vec::new();
        let mut offsets = vec![vec![0; factors.len()]; factors.len()];
        for left in 0..factors.len() {
            for right in left + 1..factors.len() {
                offsets[left][right] = pairs.len();
                for left_value in 0..factors[left].values.len() {
                    for right_value in 0..factors[right].values.len() {
                        pairs.push(Pair {
                            left,
                            left_value,
                            right,
                            right_value,
                            state: "unknown",
                            cases: vec![],
                        });
                    }
                }
            }
        }
        Self { pairs, offsets }
    }

    fn row_pairs(&self, compiled: &Compiled, row: &[usize]) -> Vec<usize> {
        let mut ids = Vec::new();
        for left in 0..row.len() {
            for right in left + 1..row.len() {
                ids.push(
                    self.offsets[left][right]
                        + row[left] * compiled.model.factors[right].values.len()
                        + row[right],
                );
            }
        }
        ids
    }
}

struct Budget<'a> {
    ctx: &'a dyn TaskContext,
    start: Instant,
    duration: Duration,
}

impl Budget<'_> {
    fn stop(&self) -> Option<&'static str> {
        if self.ctx.is_cancelled() {
            Some("cancelled")
        } else if self.start.elapsed() >= self.duration {
            Some("timeLimit")
        } else {
            None
        }
    }
}

/// Stable rank used only to break equal greedy scores; no platform RNG/hash.
fn rank(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

fn seed_hash(seed: &str) -> u64 {
    seed.bytes().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
    })
}

fn row_at(compiled: &Compiled, mut ordinal: usize) -> Vec<usize> {
    let mut row = vec![0; compiled.model.factors.len()];
    for f in (0..row.len()).rev() {
        row[f] = ordinal % compiled.model.factors[f].values.len();
        ordinal /= compiled.model.factors[f].values.len();
    }
    row
}

/// Verifies cases directly against constraints, then checks each pair against
/// actual case columns. It does not reuse greedy scores or its covered flags.
fn verify(
    compiled: &Compiled,
    cases: &[Vec<usize>],
    pairs: &mut [Pair],
    feasible: &[bool],
    complete: bool,
) -> PluginResult<()> {
    for row in cases {
        if row.len() != compiled.model.factors.len()
            || row
                .iter()
                .enumerate()
                .any(|(f, &v)| v >= compiled.model.factors[f].values.len())
            || !compiled.allowed(row)
        {
            return Err(error("invalid_case"));
        }
    }
    for (id, pair) in pairs.iter_mut().enumerate() {
        pair.cases = cases
            .iter()
            .enumerate()
            .filter_map(|(i, row)| {
                (row[pair.left] == pair.left_value && row[pair.right] == pair.right_value)
                    .then_some(i + 1)
            })
            .collect();
        pair.state = if !pair.cases.is_empty() {
            "covered"
        } else if feasible[id] {
            "uncovered"
        } else if complete {
            "infeasible"
        } else {
            "unknown"
        };
    }
    Ok(())
}

pub fn imported_cases(
    compiled: &Compiled,
    maps: Vec<BTreeMap<String, Value>>,
) -> PluginResult<Vec<Vec<usize>>> {
    if maps.len() > MAX_CASES {
        return Err(error("invalid_case"));
    }
    maps.into_iter()
        .map(|map| {
            if map.len() != compiled.model.factors.len() {
                return Err(error("invalid_case"));
            }
            compiled
                .model
                .factors
                .iter()
                .map(|factor| {
                    let value = map.get(&factor.name).ok_or_else(|| error("invalid_case"))?;
                    let canonical = scalar(value)?.1;
                    factor
                        .values
                        .iter()
                        .position(|v| v == &canonical)
                        .ok_or_else(|| error("invalid_case"))
                })
                .collect()
        })
        .collect()
}

pub fn solve(
    compiled: Compiled,
    seed: String,
    limits: Limits,
    imported: Option<Vec<Vec<usize>>>,
    ctx: &dyn TaskContext,
) -> PluginResult<Report> {
    limits.validate()?;
    let seed = seed.trim().to_owned();
    if seed.len() > 80 {
        return Err(error("seed"));
    }
    let budget = Budget {
        ctx,
        start: Instant::now(),
        duration: Duration::from_millis(limits.budget_ms),
    };
    let mut catalog = Catalog::new(&compiled);
    let mut inverted = vec![Vec::<usize>::new(); catalog.pairs.len()];
    let mut candidates = Vec::new();
    let mut enumerated = 0;
    let mut stop = None;
    ctx.stage("enumerate");
    ctx.log(
        LogLevel::Info,
        "pairwise.enumerate",
        serde_json::json!({ "total": compiled.combinations }),
    );
    for ordinal in 0..compiled.combinations {
        if ordinal >= limits.max_combinations {
            stop = Some("combinationLimit");
            break;
        }
        if ordinal % 128 == 0 {
            stop = budget.stop();
            if stop.is_some() {
                break;
            }
            ctx.progress(ordinal as u64, compiled.combinations as u64);
        }
        let row = row_at(&compiled, ordinal);
        enumerated += 1;
        if compiled.allowed(&row) {
            let id = candidates.len();
            for pair in catalog.row_pairs(&compiled, &row) {
                inverted[pair].push(id);
            }
            candidates.push(row);
        }
    }
    let feasibility_complete = enumerated == compiled.combinations;
    let feasible = inverted.iter().map(|v| !v.is_empty()).collect::<Vec<_>>();
    let valid_combinations = candidates.len();
    let importing = imported.is_some();
    let mut cases = imported.unwrap_or_default();
    // Imported cases are assessed as supplied, never silently regenerated.
    if !importing {
        ctx.stage("cover");
        let mut scores = vec![
            compiled.model.factors.len() * (compiled.model.factors.len() - 1) / 2;
            candidates.len()
        ];
        let mut covered = vec![false; catalog.pairs.len()];
        let hash = seed_hash(&seed);
        while stop.is_none() {
            stop = budget.stop();
            if stop.is_some() {
                break;
            }
            let best = scores
                .iter()
                .enumerate()
                .max_by_key(|&(i, score)| (*score, rank(hash ^ i as u64)));
            let Some((id, &score)) = best else {
                break;
            };
            if score == 0 {
                break;
            }
            if cases.len() == limits.max_cases {
                stop = Some("caseLimit");
                break;
            }
            let row = &candidates[id];
            cases.push(row.clone());
            for pair in catalog.row_pairs(&compiled, row) {
                if !covered[pair] {
                    covered[pair] = true;
                    for &candidate in &inverted[pair] {
                        scores[candidate] -= 1;
                    }
                }
            }
            ctx.progress(
                covered.iter().filter(|v| **v).count() as u64,
                feasible.iter().filter(|v| **v).count() as u64,
            );
        }
    }
    ctx.stage("verify");
    verify(
        &compiled,
        &cases,
        &mut catalog.pairs,
        &feasible,
        feasibility_complete,
    )?;
    let (status, reason) = if feasibility_complete && valid_combinations == 0 {
        ("unsatisfiable", None)
    } else if stop.is_none()
        && feasibility_complete
        && catalog
            .pairs
            .iter()
            .all(|p| matches!(p.state, "covered" | "infeasible"))
    {
        ("complete", None)
    } else {
        ("incomplete", stop.or(Some("uncovered")))
    };
    ctx.log(
        if status == "complete" {
            LogLevel::Info
        } else {
            LogLevel::Warn
        },
        "pairwise.result",
        serde_json::json!({ "status": status, "cases": cases.len() }),
    );
    ctx.stage("done");
    Ok(Report {
        compiled,
        seed,
        limits,
        cases,
        pairs: catalog.pairs,
        enumerated,
        valid_combinations,
        feasibility_complete,
        status,
        reason,
    })
}

#[cfg(test)]
#[path = "solver_test.rs"]
mod tests;
