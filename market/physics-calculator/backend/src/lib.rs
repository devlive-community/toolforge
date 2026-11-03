//! 物理公式计算插件后端：按公式卡求解目标量。
//!
//! 每张卡固定国际单位制（变量与单位一一对应，不存在混用单位的输入），
//! 缺一个已知量或多于一个未知量都会被拒绝，避免给出看似正确的错数。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::f64::consts::PI;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct PhysicsCalculator {
    manifest: Manifest,
}

impl Default for PhysicsCalculator {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for PhysicsCalculator {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "solve" => to_value(solve(parse_args(args)?)?),
            "constants" => to_value(constants()),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(PhysicsCalculator);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolveArgs {
    /// 公式卡 id
    pub card: String,
    /// 要求解的变量名
    pub target: String,
    /// 已知量：变量名 → 数值（国际单位制）
    pub given: std::collections::HashMap<String, f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub card: String,
    pub target: String,
    pub value: f64,
    /// 单位键（units.<key>）
    pub unit: &'static str,
    /// 公式原文
    pub formula: &'static str,
    /// 代入过程
    pub substituted: String,
}

/// 常量表（国际单位制）
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Constant {
    key: &'static str,
    value: f64,
    unit: &'static str,
}

fn constants() -> Vec<Constant> {
    vec![
        Constant {
            key: "g",
            value: 9.8,
            unit: "m/s2",
        },
        Constant {
            key: "c",
            value: 299_792_458.0,
            unit: "m/s",
        },
        Constant {
            key: "e_charge",
            value: 1.602_176_634e-19,
            unit: "C",
        },
        Constant {
            key: "G",
            value: 6.674_3e-11,
            unit: "N·m2/kg2",
        },
        Constant {
            key: "atm",
            value: 101_325.0,
            unit: "Pa",
        },
        Constant {
            key: "water_c",
            value: 4186.0,
            unit: "J/(kg·K)",
        },
    ]
}

/// 卡片定义：变量与单位、公式原文
struct Card {
    vars: &'static [(&'static str, &'static str)],
    formula: &'static str,
}

static CARDS: &[(&str, Card)] = &[
    (
        "kinematics_v",
        Card {
            vars: &[("v", "m/s"), ("v0", "m/s"), ("a", "m/s2"), ("t", "s")],
            formula: "v = v0 + at",
        },
    ),
    (
        "kinematics_s",
        Card {
            vars: &[("s", "m"), ("v0", "m/s"), ("a", "m/s2"), ("t", "s")],
            formula: "s = v0t + ½at²",
        },
    ),
    (
        "kinematics_v2",
        Card {
            vars: &[("v", "m/s"), ("v0", "m/s"), ("a", "m/s2"), ("s", "m")],
            formula: "v² = v0² + 2as",
        },
    ),
    (
        "newton",
        Card {
            vars: &[("f", "N"), ("m", "kg"), ("a", "m/s2")],
            formula: "F = ma",
        },
    ),
    (
        "friction",
        Card {
            vars: &[("f", "N"), ("mu", "1"), ("n", "N")],
            formula: "f = μN",
        },
    ),
    (
        "work",
        Card {
            vars: &[("w", "J"), ("f", "N"), ("s", "m")],
            formula: "W = Fs",
        },
    ),
    (
        "power",
        Card {
            vars: &[("p", "W"), ("w", "J"), ("t", "s")],
            formula: "P = W / t",
        },
    ),
    (
        "potential",
        Card {
            vars: &[("ep", "J"), ("m", "kg"), ("g", "m/s2"), ("h", "m")],
            formula: "Ep = mgh",
        },
    ),
    (
        "kinetic",
        Card {
            vars: &[("ek", "J"), ("m", "kg"), ("v", "m/s")],
            formula: "Ek = ½mv²",
        },
    ),
    (
        "ohm",
        Card {
            vars: &[("u", "V"), ("i", "A"), ("r", "Ω")],
            formula: "U = IR",
        },
    ),
    (
        "electric_power",
        Card {
            vars: &[("p", "W"), ("u", "V"), ("i", "A")],
            formula: "P = UI",
        },
    ),
    (
        "electric_power_r",
        Card {
            vars: &[("p", "W"), ("i", "A"), ("r", "Ω")],
            formula: "P = I²R",
        },
    ),
    (
        "series_r",
        Card {
            vars: &[("r", "Ω"), ("r1", "Ω"), ("r2", "Ω")],
            formula: "R = R1 + R2",
        },
    ),
    (
        "parallel_r",
        Card {
            vars: &[("r", "Ω"), ("r1", "Ω"), ("r2", "Ω")],
            formula: "1/R = 1/R1 + 1/R2",
        },
    ),
    (
        "refraction",
        Card {
            vars: &[("n", "1"), ("c", "m/s"), ("v", "m/s")],
            formula: "n = c / v",
        },
    ),
    (
        "heat",
        Card {
            vars: &[("q", "J"), ("m", "kg"), ("cc", "J/(kg·K)"), ("dt", "K")],
            formula: "Q = cmΔt",
        },
    ),
];

fn err(code: &str, key: &str, value: impl Into<Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

pub fn solve(args: SolveArgs) -> PluginResult<Output> {
    let (_, card) = CARDS
        .iter()
        .find(|(id, _)| *id == args.card)
        .ok_or_else(|| err("physics.unknown_card", "card", args.card.clone()))?;
    if !card.vars.iter().any(|(name, _)| *name == args.target) {
        return Err(err(
            "physics.unknown_variable",
            "target",
            args.target.clone(),
        ));
    }
    // 已知量必须恰好覆盖除 target 外的全部变量
    let mut given = args.given.clone();
    for (name, _) in card.vars {
        if *name == args.target {
            continue;
        }
        let value = given
            .remove(*name)
            .ok_or_else(|| err("physics.missing_given", "name", *name))?;
        if !value.is_finite() {
            return Err(err("physics.bad_number", "name", *name));
        }
    }
    if !given.is_empty() {
        return Err(err(
            "physics.extra_given",
            "name",
            given.keys().next().cloned().unwrap_or_default(),
        ));
    }
    let get = |name: &str| -> f64 { args.given.get(name).copied().unwrap_or(f64::NAN) };
    let v = get("v");
    let v0 = get("v0");
    let a = get("a");
    let t = get("t");
    let s = get("s");
    let f = get("f");
    let m = get("m");
    let mu = get("mu");
    let n = get("n");
    let w = get("w");
    let p = get("p");
    let ep = get("ep");
    let ek = get("ek");
    let u = get("u");
    let i = get("i");
    let r = get("r");
    let r1 = get("r1");
    let r2 = get("r2");
    let c = get("c");
    let h = get("h");
    let g = get("g");
    let q = get("q");
    let cc = get("cc");
    let dt = get("dt");
    let zero = |value: f64, name: &str| -> PluginResult<f64> {
        if value == 0.0 {
            return Err(err("physics.divide_by_zero", "name", name));
        }
        Ok(value)
    };
    let (value, substituted) = match (args.card.as_str(), args.target.as_str()) {
        ("kinematics_v", "v") => (v0 + a * t, format!("v = {v0} + {a} × {t}")),
        ("kinematics_v", "v0") => (v - a * t, format!("v0 = {v} − {a} × {t}")),
        ("kinematics_v", "a") => ((v - v0) / zero(t, "t")?, format!("a = ({v} − {v0}) / {t}")),
        ("kinematics_v", "t") => ((v - v0) / zero(a, "a")?, format!("t = ({v} − {v0}) / {a}")),
        ("kinematics_s", "s") => (
            v0 * t + 0.5 * a * t * t,
            format!("s = {v0} × {t} + ½ × {a} × {t}²"),
        ),
        ("kinematics_s", "v0") => (
            (s - 0.5 * a * t * t) / zero(t, "t")?,
            format!("v0 = ({s} − ½ × {a} × {t}²) / {t}"),
        ),
        ("kinematics_s", "a") => (
            2.0 * (s - v0 * t) / zero(t * t, "t")?,
            format!("a = 2 × ({s} − {v0} × {t}) / {t}²"),
        ),
        ("kinematics_s", "t") => {
            // s = v0t + ½at² → t = (−v0 ± √(v0² + 2as)) / a；取正根
            let disc = v0 * v0 + 2.0 * a * s;
            if disc < 0.0 {
                return Err(err("physics.no_real_solution", "card", "kinematics_s"));
            }
            let root = disc.sqrt();
            let t1 = (-v0 + root) / a;
            let t2 = (-v0 - root) / a;
            let picked = if t1 >= 0.0 { t1 } else { t2 };
            (picked, format!("t = (−{v0} ± √({disc})) / {a} → {picked}"))
        }
        ("kinematics_v2", "v") => (
            (v0 * v0 + 2.0 * a * s).sqrt(),
            format!("v = √({v0}² + 2 × {a} × {s})"),
        ),
        ("kinematics_v2", "v0") => (
            (v * v - 2.0 * a * s).sqrt(),
            format!("v0 = √({v}² − 2 × {a} × {s})"),
        ),
        ("kinematics_v2", "a") => (
            (v * v - v0 * v0) / zero(2.0 * s, "s")?,
            format!("a = ({v}² − {v0}²) / (2 × {s})"),
        ),
        ("kinematics_v2", "s") => (
            (v * v - v0 * v0) / zero(2.0 * a, "a")?,
            format!("s = ({v}² − {v0}²) / (2 × {a})"),
        ),
        ("newton", "f") => (m * a, format!("F = {m} × {a}")),
        ("newton", "m") => (f / zero(a, "a")?, format!("m = {f} / {a}")),
        ("newton", "a") => (f / zero(m, "m")?, format!("a = {f} / {m}")),
        ("friction", "f") => (mu * n, format!("f = {mu} × {n}")),
        ("friction", "mu") => (f / zero(n, "N")?, format!("μ = {f} / {n}")),
        ("friction", "n") => (f / zero(mu, "μ")?, format!("N = {f} / {mu}")),
        ("work", "w") => (f * s, format!("W = {f} × {s}")),
        ("work", "f") => (w / zero(s, "s")?, format!("F = {w} / {s}")),
        ("work", "s") => (w / zero(f, "F")?, format!("s = {w} / {f}")),
        ("power", "p") => (w / zero(t, "t")?, format!("P = {w} / {t}")),
        ("power", "w") => (p * t, format!("W = {p} × {t}")),
        ("power", "t") => (w / zero(p, "P")?, format!("t = {w} / {p}")),
        ("potential", "ep") => (m * g * h, format!("Ep = {m} × {g} × {h}")),
        ("potential", "m") => (ep / zero(g * h, "gh")?, format!("m = {ep} / ({g} × {h})")),
        ("potential", "h") => (ep / zero(m * g, "mg")?, format!("h = {ep} / ({m} × {g})")),
        ("potential", "g") => (ep / zero(m * h, "mh")?, format!("g = {ep} / ({m} × {h})")),
        ("kinetic", "ek") => (0.5 * m * v * v, format!("Ek = ½ × {m} × {v}²")),
        ("kinetic", "m") => (
            2.0 * ek / zero(v * v, "v²")?,
            format!("m = 2 × {ek} / {v}²"),
        ),
        ("kinetic", "v") => (
            (2.0 * ek / zero(m, "m")?).sqrt(),
            format!("v = √(2 × {ek} / {m})"),
        ),
        ("ohm", "u") => (i * r, format!("U = {i} × {r}")),
        ("ohm", "i") => (u / zero(r, "R")?, format!("I = {u} / {r}")),
        ("ohm", "r") => (u / zero(i, "I")?, format!("R = {u} / I")),
        ("electric_power", "p") => (u * i, format!("P = {u} × {i}")),
        ("electric_power", "u") => (p / zero(i, "I")?, format!("U = {p} / {i}")),
        ("electric_power", "i") => (p / zero(u, "U")?, format!("I = {p} / {u}")),
        ("electric_power_r", "p") => (i * i * r, format!("P = {i}² × {r}")),
        ("electric_power_r", "i") => ((p / zero(r, "R")?).sqrt(), format!("I = √({p} / {r})")),
        ("electric_power_r", "r") => (p / zero(i * i, "I²")?, format!("R = {p} / {i}²")),
        ("series_r", "r") => (r1 + r2, format!("R = {r1} + {r2}")),
        ("series_r", "r1") => (r - r2, format!("R1 = {r} − {r2}")),
        ("series_r", "r2") => (r - r1, format!("R2 = {r} − {r1}")),
        ("parallel_r", "r") => (
            1.0 / (1.0 / zero(r1, "R1")? + 1.0 / zero(r2, "R2")?),
            format!("R = 1 / (1/{r1} + 1/{r2})"),
        ),
        ("parallel_r", "r1") => (
            1.0 / (1.0 / zero(r, "R")? - 1.0 / zero(r2, "R2")?),
            format!("R1 = 1 / (1/{r} − 1/{r2})"),
        ),
        ("parallel_r", "r2") => (
            1.0 / (1.0 / zero(r, "R")? - 1.0 / zero(r1, "R1")?),
            format!("R2 = 1 / (1/{r} − 1/{r1})"),
        ),
        ("refraction", "n") => (c / zero(v, "v")?, format!("n = {c} / {v}")),
        ("refraction", "v") => (c / zero(n, "n")?, format!("v = {c} / {n}")),
        ("refraction", "c") => (n * v, format!("c = {n} × {v}")),
        ("heat", "q") => (m * cc * dt, format!("Q = {m} × {cc} × {dt}")),
        ("heat", "m") => (
            q / zero(cc * dt, "cΔt")?,
            format!("m = {q} / ({cc} × {dt})"),
        ),
        ("heat", "dt") => (q / zero(m * cc, "mc")?, format!("Δt = {q} / ({m} × {cc})")),
        _ => return Err(err("physics.unknown_target", "target", args.target.clone())),
    };
    if !value.is_finite() {
        return Err(err("physics.not_finite", "target", args.target.clone()));
    }
    let unit = card
        .vars
        .iter()
        .find(|(name, _)| *name == args.target)
        .map(|(_, unit)| *unit)
        .unwrap_or("1");
    Ok(Output {
        card: args.card,
        target: args.target,
        value,
        unit,
        formula: card.formula,
        substituted,
    })
}

// PI 供后续扩展使用，避免未使用告警
const _: f64 = PI;

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
