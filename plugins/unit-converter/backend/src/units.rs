use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

/// 单位：`factor` 为换算到本类基准单位的倍数（温度除外）
pub struct Unit {
    pub id: &'static str,
    pub symbol: &'static str,
    pub factor: f64,
}

pub struct Category {
    pub id: &'static str,
    pub base: &'static str,
    pub units: &'static [Unit],
}

const fn u(id: &'static str, symbol: &'static str, factor: f64) -> Unit {
    Unit { id, symbol, factor }
}

const PI: f64 = std::f64::consts::PI;

pub static CATEGORIES: &[Category] = &[
    Category {
        id: "length",
        base: "m",
        units: &[
            u("nm", "nm", 1e-9),
            u("um", "µm", 1e-6),
            u("mm", "mm", 1e-3),
            u("cm", "cm", 1e-2),
            u("m", "m", 1.0),
            u("km", "km", 1e3),
            u("in", "in", 0.0254),
            u("ft", "ft", 0.3048),
            u("yd", "yd", 0.9144),
            u("mi", "mi", 1609.344),
            u("nmi", "nmi", 1852.0),
            u("li", "里", 500.0),
            u("zhang", "丈", 10.0 / 3.0),
            u("chi", "尺", 1.0 / 3.0),
            u("cun", "寸", 1.0 / 30.0),
        ],
    },
    Category {
        id: "mass",
        base: "kg",
        units: &[
            u("mg", "mg", 1e-6),
            u("g", "g", 1e-3),
            u("kg", "kg", 1.0),
            u("t", "t", 1e3),
            u("ct", "ct", 2e-4),
            u("oz", "oz", 0.028_349_523_125),
            u("lb", "lb", 0.453_592_37),
            u("st", "st", 6.350_293_18),
            u("jin", "斤", 0.5),
            u("liang", "两", 0.05),
        ],
    },
    Category {
        id: "temperature",
        base: "c",
        units: &[
            u("c", "°C", 1.0),
            u("f", "°F", 1.0),
            u("k", "K", 1.0),
            u("r", "°R", 1.0),
        ],
    },
    Category {
        id: "area",
        base: "m2",
        units: &[
            u("mm2", "mm²", 1e-6),
            u("cm2", "cm²", 1e-4),
            u("m2", "m²", 1.0),
            u("ha", "ha", 1e4),
            u("km2", "km²", 1e6),
            u("in2", "in²", 0.000_645_16),
            u("ft2", "ft²", 0.092_903_04),
            u("yd2", "yd²", 0.836_127_36),
            u("acre", "ac", 4_046.856_422_4),
            u("mi2", "mi²", 2_589_988.110_336),
            u("mu", "亩", 10_000.0 / 15.0),
        ],
    },
    Category {
        id: "volume",
        base: "l",
        units: &[
            u("ml", "mL", 1e-3),
            u("l", "L", 1.0),
            u("m3", "m³", 1e3),
            u("cm3", "cm³", 1e-3),
            u("in3", "in³", 0.016_387_064),
            u("ft3", "ft³", 28.316_846_592),
            u("tsp", "tsp", 0.004_928_921_593_75),
            u("tbsp", "tbsp", 0.014_786_764_781_25),
            u("floz", "fl oz", 0.029_573_529_562_5),
            u("cup", "cup", 0.236_588_236_5),
            u("pt", "pt", 0.473_176_473),
            u("qt", "qt", 0.946_352_946),
            u("gal", "gal", 3.785_411_784),
            u("galuk", "gal (UK)", 4.546_09),
        ],
    },
    Category {
        id: "speed",
        base: "mps",
        units: &[
            u("mps", "m/s", 1.0),
            u("kmh", "km/h", 1.0 / 3.6),
            u("mph", "mph", 0.447_04),
            u("fps", "ft/s", 0.3048),
            u("kn", "kn", 1852.0 / 3600.0),
        ],
    },
    Category {
        id: "time",
        base: "s",
        units: &[
            u("ns", "ns", 1e-9),
            u("us", "µs", 1e-6),
            u("ms", "ms", 1e-3),
            u("s", "s", 1.0),
            u("min", "min", 60.0),
            u("h", "h", 3600.0),
            u("d", "d", 86_400.0),
            u("wk", "wk", 604_800.0),
            // 公历平均月与年
            u("mo", "mo", 2_629_746.0),
            u("yr", "yr", 31_556_952.0),
        ],
    },
    Category {
        id: "data",
        base: "byte",
        units: &[
            u("bit", "bit", 0.125),
            u("byte", "B", 1.0),
            u("kb", "KB", 1e3),
            u("mb", "MB", 1e6),
            u("gb", "GB", 1e9),
            u("tb", "TB", 1e12),
            u("pb", "PB", 1e15),
            u("kib", "KiB", 1024.0),
            u("mib", "MiB", 1_048_576.0),
            u("gib", "GiB", 1_073_741_824.0),
            u("tib", "TiB", 1_099_511_627_776.0),
            u("pib", "PiB", 1_125_899_906_842_624.0),
        ],
    },
    Category {
        id: "datarate",
        base: "bps",
        units: &[
            u("bps", "bit/s", 1.0),
            u("kbps", "kbit/s", 1e3),
            u("mbps", "Mbit/s", 1e6),
            u("gbps", "Gbit/s", 1e9),
            u("byteps", "B/s", 8.0),
            u("kbyteps", "KB/s", 8e3),
            u("mbyteps", "MB/s", 8e6),
            u("gbyteps", "GB/s", 8e9),
        ],
    },
    Category {
        id: "pressure",
        base: "pa",
        units: &[
            u("pa", "Pa", 1.0),
            u("hpa", "hPa", 100.0),
            u("kpa", "kPa", 1e3),
            u("mpa", "MPa", 1e6),
            u("bar", "bar", 1e5),
            u("mbar", "mbar", 100.0),
            u("atm", "atm", 101_325.0),
            u("psi", "psi", 6_894.757_293_168),
            u("mmhg", "mmHg", 133.322_387_415),
            u("torr", "Torr", 101_325.0 / 760.0),
        ],
    },
    Category {
        id: "energy",
        base: "j",
        units: &[
            u("j", "J", 1.0),
            u("kj", "kJ", 1e3),
            u("cal", "cal", 4.184),
            u("kcal", "kcal", 4_184.0),
            u("wh", "Wh", 3_600.0),
            u("kwh", "kWh", 3.6e6),
            u("ev", "eV", 1.602_176_634e-19),
            u("btu", "BTU", 1_055.055_852_62),
            u("ftlb", "ft·lbf", 1.355_817_948_331_400_4),
        ],
    },
    Category {
        id: "power",
        base: "w",
        units: &[
            u("w", "W", 1.0),
            u("kw", "kW", 1e3),
            u("mw", "MW", 1e6),
            u("hp", "hp", 745.699_871_582_270_2),
            u("ps", "PS", 735.498_75),
            u("btuh", "BTU/h", 0.293_071_070_172_222_2),
        ],
    },
    Category {
        id: "angle",
        base: "deg",
        units: &[
            u("deg", "°", 1.0),
            u("rad", "rad", 180.0 / PI),
            u("grad", "gon", 0.9),
            u("arcmin", "′", 1.0 / 60.0),
            u("arcsec", "″", 1.0 / 3600.0),
            u("turn", "turn", 360.0),
        ],
    },
    Category {
        id: "frequency",
        base: "hz",
        units: &[
            u("hz", "Hz", 1.0),
            u("khz", "kHz", 1e3),
            u("mhz", "MHz", 1e6),
            u("ghz", "GHz", 1e9),
            u("rpm", "rpm", 1.0 / 60.0),
        ],
    },
];

#[derive(Debug, Serialize)]
pub struct UnitInfo {
    pub id: &'static str,
    pub symbol: &'static str,
}

#[derive(Debug, Serialize)]
pub struct CategoryInfo {
    pub id: &'static str,
    pub base: &'static str,
    pub units: Vec<UnitInfo>,
}

/// 所有分类与单位（前端据此渲染，不在界面里写死单位表）
pub fn catalog() -> Vec<CategoryInfo> {
    CATEGORIES
        .iter()
        .map(|c| CategoryInfo {
            id: c.id,
            base: c.base,
            units: c
                .units
                .iter()
                .map(|u| UnitInfo {
                    id: u.id,
                    symbol: u.symbol,
                })
                .collect(),
        })
        .collect()
}

fn default_precision() -> u8 {
    10
}

#[derive(Deserialize)]
pub struct Args {
    category: String,
    from: String,
    value: String,
    /// 有效数字位数
    #[serde(default = "default_precision")]
    precision: u8,
}

#[derive(Debug, Serialize)]
pub struct Converted {
    pub unit: &'static str,
    pub symbol: &'static str,
    /// 按有效数字格式化后的值
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct Output {
    pub category: &'static str,
    pub input: String,
    pub results: Vec<Converted>,
}

/// 解析数值：允许千分位逗号、空格、下划线与科学计数法
pub fn parse_value(raw: &str) -> PluginResult<f64> {
    let clean: String = raw
        .trim()
        .chars()
        .filter(|c| !matches!(c, ',' | '_' | ' ' | '\u{a0}'))
        .collect();
    if clean.is_empty() {
        return Err(PluginError::new("unit.empty"));
    }
    clean
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| PluginError::new("unit.invalid_number").with("value", raw.trim()))
}

/// 按有效数字格式化：极大或极小的数用科学计数法，去掉多余的零
pub fn format(value: f64, precision: u8) -> String {
    let precision = precision.clamp(1, 17) as usize;
    if value == 0.0 {
        return "0".into();
    }
    let magnitude = value.abs().log10().floor() as i32;
    if !(-6..15).contains(&magnitude) {
        let formatted = format!("{:.*e}", precision - 1, value);
        let (mantissa, exponent) = formatted.split_once('e').unwrap_or((&formatted, "0"));
        let mantissa = trim_zeros(mantissa);
        return format!("{mantissa}e{exponent}");
    }
    let decimals = (precision as i32 - 1 - magnitude).max(0) as usize;
    let rounded = format!("{value:.decimals$}");
    // 舍入可能产生 -0
    let trimmed = trim_zeros(&rounded);
    if trimmed == "-0" { "0".into() } else { trimmed }
}

fn trim_zeros(text: &str) -> String {
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text.to_owned()
    }
}

fn to_celsius(unit: &str, v: f64) -> f64 {
    match unit {
        "f" => (v - 32.0) * 5.0 / 9.0,
        "k" => v - 273.15,
        "r" => (v - 491.67) * 5.0 / 9.0,
        _ => v,
    }
}

fn from_celsius(unit: &str, c: f64) -> f64 {
    match unit {
        "f" => c * 9.0 / 5.0 + 32.0,
        "k" => c + 273.15,
        "r" => (c + 273.15) * 9.0 / 5.0,
        _ => c,
    }
}

pub fn convert(args: Args) -> PluginResult<Output> {
    let category = CATEGORIES
        .iter()
        .find(|c| c.id == args.category)
        .ok_or_else(|| {
            PluginError::new("unit.unknown_category").with("category", args.category.as_str())
        })?;
    let from = category
        .units
        .iter()
        .find(|u| u.id == args.from)
        .ok_or_else(|| PluginError::new("unit.unknown_unit").with("unit", args.from.as_str()))?;
    let value = parse_value(&args.value)?;

    let results = if category.id == "temperature" {
        let celsius = to_celsius(from.id, value);
        if celsius < -273.15 - 1e-9 {
            return Err(PluginError::new("unit.below_absolute_zero"));
        }
        category
            .units
            .iter()
            .map(|u| Converted {
                unit: u.id,
                symbol: u.symbol,
                value: format(from_celsius(u.id, celsius), args.precision),
            })
            .collect()
    } else {
        let base = value * from.factor;
        category
            .units
            .iter()
            .map(|u| Converted {
                unit: u.id,
                symbol: u.symbol,
                value: format(base / u.factor, args.precision),
            })
            .collect()
    };
    Ok(Output {
        category: category.id,
        input: format(value, args.precision),
        results,
    })
}

#[cfg(test)]
#[path = "units_test.rs"]
mod tests;
