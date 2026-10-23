use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult, TaskContext};

pub const CENT_DIVISOR: u128 = 10_000_000_000_000;
pub fn error(code: &str) -> PluginError {
    PluginError::new(format!("electricity-cost.{code}"))
}
pub fn check(ctx: &dyn TaskContext) -> PluginResult<()> {
    if ctx.is_cancelled() {
        Err(tf_plugin_api::cancelled())
    } else {
        Ok(())
    }
}
/// Strict decimal strings keep API inputs and bill rounding independent of IEEE-754.
pub fn decimal(text: &str, places: u32, max: u128, field: &str) -> PluginResult<u128> {
    let invalid = || error("number").with("field", field);
    if text.is_empty() || text.len() > 32 {
        return Err(invalid());
    }
    let mut parts = text.split('.');
    let whole = parts.next().unwrap_or_default();
    let fraction = parts.next();
    if whole.is_empty() || !whole.bytes().all(|b| b.is_ascii_digit()) || parts.next().is_some() {
        return Err(invalid());
    }
    let scale = 10u128.pow(places);
    let mut n = whole
        .parse::<u128>()
        .map_err(|_| invalid())?
        .checked_mul(scale)
        .ok_or_else(invalid)?;
    if let Some(f) = fraction {
        if f.is_empty() || f.len() > places as usize || !f.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid());
        }
        n = n
            .checked_add(
                f.parse::<u128>().map_err(|_| invalid())? * 10u128.pow(places - f.len() as u32),
            )
            .ok_or_else(invalid)?;
    }
    if n > max * scale {
        return Err(invalid());
    }
    Ok(n)
}
pub fn fixed(n: u128, places: u32) -> String {
    if places == 0 {
        return n.to_string();
    }
    let scale = 10u128.pow(places);
    format!(
        "{}.{:0width$}",
        n / scale,
        n % scale,
        width = places as usize
    )
}
pub fn energy(n: u128) -> String {
    fixed(n, 9)
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}
pub fn money(n: u128) -> String {
    fixed(n, 2)
}
pub fn round_div(n: u128, d: u128) -> u128 {
    n / d + u128::from(n % d >= d.div_ceil(2))
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Usage {
    pub power: String,
    pub unit: String,
    pub peak: String,
    pub shoulder: String,
    pub valley: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Appliance {
    pub id: String,
    pub name: String,
    pub quantity: u32,
    pub usage: Usage,
    pub alternative: Option<Usage>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Tier {
    pub up_to: Option<String>,
    pub rate: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Pricing {
    Flat {
        rate: String,
    },
    Tiered {
        tiers: Vec<Tier>,
    },
    Tou {
        peak: String,
        shoulder: String,
        valley: String,
    },
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Tariff {
    pub id: String,
    pub name: String,
    pub pricing: Pricing,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    pub days: u32,
    pub currency: String,
    pub appliances: Vec<Appliance>,
    pub tariffs: Vec<Tariff>,
}
pub fn label(value: &str) -> bool {
    !value.trim().is_empty() && value.chars().count() <= 80 && !value.chars().any(char::is_control)
}
pub fn id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
}
pub fn validate_tariff(t: &Tariff) -> PluginResult<()> {
    if !id(&t.id) || !label(&t.name) {
        return Err(error("label"));
    }
    let rate = |v: &str| decimal(v, 6, 1000, "rate").map(|_| ());
    match &t.pricing {
        Pricing::Flat { rate: v } => rate(v)?,
        Pricing::Tou {
            peak,
            shoulder,
            valley,
        } => {
            rate(peak)?;
            rate(shoulder)?;
            rate(valley)?;
        }
        Pricing::Tiered { tiers } => {
            if tiers.is_empty()
                || tiers.len() > 10
                || tiers.last().is_none_or(|t| t.up_to.is_some())
            {
                return Err(error("tiers"));
            }
            let mut last = 0;
            for (i, t) in tiers.iter().enumerate() {
                rate(&t.rate)?;
                if let Some(value) = &t.up_to {
                    let cap = decimal(value, 3, 1_000_000_000, "threshold")?;
                    if cap <= last {
                        return Err(error("tiers"));
                    }
                    last = cap;
                } else if i + 1 != tiers.len() {
                    return Err(error("tiers"));
                }
            }
        }
    }
    Ok(())
}
pub fn usage_energy(u: &Usage, quantity: u32, days: u32) -> PluginResult<[u128; 3]> {
    let watts = match u.unit.as_str() {
        "w" => decimal(&u.power, 3, 1_000_000, "power")?,
        "kw" => decimal(&u.power, 3, 1000, "power")? * 1000,
        _ => return Err(error("unit")),
    };
    let hours = [&u.peak, &u.shoulder, &u.valley].map(|h| decimal(h, 3, 24, "hours"));
    let [a, b, c] = hours;
    let hours = [a?, b?, c?];
    if hours.iter().sum::<u128>() > 24_000 {
        return Err(error("hours"));
    }
    Ok(hours.map(|h| watts * h * u128::from(quantity) * u128::from(days)))
}
pub fn validate(input: &Input) -> PluginResult<()> {
    if !(1..=31).contains(&input.days)
        || !label(&input.currency)
        || input.currency.chars().count() > 12
        || input.appliances.is_empty()
        || input.appliances.len() > 100
        || input.tariffs.is_empty()
        || input.tariffs.len() > 8
    {
        return Err(error("limits"));
    }
    let mut ids = std::collections::HashSet::new();
    for a in &input.appliances {
        if !id(&a.id) || !ids.insert(&a.id) || !label(&a.name) || !(1..=1000).contains(&a.quantity)
        {
            return Err(error("label"));
        }
        usage_energy(&a.usage, a.quantity, input.days)?;
        if let Some(u) = &a.alternative {
            usage_energy(u, a.quantity, input.days)?;
        }
    }
    ids.clear();
    for t in &input.tariffs {
        if !ids.insert(&t.id) {
            return Err(error("label"));
        }
        validate_tariff(t)?;
    }
    Ok(())
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Band {
    pub index: usize,
    pub up_to: Option<String>,
    pub rate: String,
    pub kwh: String,
    pub cost: String,
    pub reached: bool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub daily_kwh: String,
    pub monthly_kwh: String,
    pub monthly_cost: String,
    pub daily_cost: String,
    pub annual_cost: String,
    pub highest_tier: Option<usize>,
    pub remaining_kwh: Option<String>,
    pub bands: Vec<Band>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub id: String,
    pub name: String,
    pub baseline_kwh: String,
    pub alternative_kwh: String,
    pub baseline_cost: String,
    pub alternative_cost: String,
    pub share_percent: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bill {
    pub tariff: Tariff,
    pub baseline: Totals,
    pub alternative: Totals,
    pub annual_saving: String,
    pub saving_direction: String,
    pub rows: Vec<Row>,
}
pub struct Report {
    pub input: Input,
    pub bills: Vec<Bill>,
}
