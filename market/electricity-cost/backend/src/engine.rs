use crate::model::*;
use tf_plugin_api::{PluginResult, TaskContext};

/// Largest-remainder allocation keeps displayed appliance cents equal to the bill.
fn allocate(total: u128, weights: &[u128]) -> Vec<u128> {
    let sum: u128 = weights.iter().sum();
    if sum == 0 {
        return vec![0; weights.len()];
    }
    let ratios: Vec<_> = weights.iter().map(|w| mul_div(total, *w, sum)).collect();
    let mut values: Vec<_> = ratios.iter().map(|p| p.0).collect();
    let mut order: Vec<_> = ratios.iter().enumerate().map(|(i, p)| (i, p.1)).collect();
    order.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let remaining = total - values.iter().sum::<u128>();
    for (i, _) in order.into_iter().take(remaining as usize) {
        values[i] += 1;
    }
    values
}
// All validated denominators are below 10^30. Long multiplication avoids
// overflowing an intermediate product even at the maximum appliance budget.
fn mul_div(a: u128, b: u128, denominator: u128) -> (u128, u128) {
    if let Some(product) = a.checked_mul(b) {
        return (product / denominator, product % denominator);
    }
    let (mut quotient, mut remainder) = (0, 0);
    for bit in (0..128).rev() {
        remainder *= 2;
        if (a >> bit) & 1 != 0 {
            remainder += b;
        }
        quotient = quotient * 2 + remainder / denominator;
        remainder %= denominator;
    }
    (quotient, remainder)
}
fn bill(pricing: &Pricing, usage: &[[u128; 3]], days: u32) -> PluginResult<(Totals, Vec<u128>)> {
    let energies: Vec<u128> = usage.iter().map(|p| p.iter().sum()).collect();
    let total: u128 = energies.iter().sum();
    let rate = |v: &str| decimal(v, 6, 1000, "rate");
    let mut bands = Vec::new();
    let mut highest = None;
    let mut remaining = None;
    let (raw, weights) = match pricing {
        Pricing::Flat { rate: r } => {
            let r = rate(r)?;
            let w: Vec<_> = energies.iter().map(|e| e * r).collect();
            (w.iter().sum(), w)
        }
        Pricing::Tou {
            peak,
            shoulder,
            valley,
        } => {
            let rates = [rate(peak)?, rate(shoulder)?, rate(valley)?];
            let w: Vec<u128> = usage
                .iter()
                .map(|p| p.iter().zip(rates).map(|(e, r)| e * r).sum())
                .collect();
            let amounts: [u128; 3] = std::array::from_fn(|i| usage.iter().map(|p| p[i]).sum());
            let costs = allocate(
                round_div(w.iter().sum(), CENT_DIVISOR),
                &amounts
                    .iter()
                    .zip(rates)
                    .map(|(e, r)| e * r)
                    .collect::<Vec<_>>(),
            );
            for i in 0..3 {
                bands.push(Band {
                    index: i,
                    up_to: None,
                    rate: fixed(rates[i], 6),
                    kwh: energy(amounts[i]),
                    cost: money(costs[i]),
                    reached: amounts[i] > 0,
                });
            }
            (w.iter().sum(), w)
        }
        Pricing::Tiered { tiers } => {
            let mut previous = 0;
            let mut raw = 0;
            let mut raw_bands = Vec::new();
            for (i, tier) in tiers.iter().enumerate() {
                let cap = tier
                    .up_to
                    .as_ref()
                    .map(|v| decimal(v, 3, 1_000_000_000, "threshold").map(|n| n * 1_000_000))
                    .transpose()?;
                let amount = total
                    .saturating_sub(previous)
                    .min(cap.map_or(u128::MAX, |c| c - previous));
                let cost = amount * rate(&tier.rate)?;
                raw += cost;
                raw_bands.push(cost);
                if amount > 0 {
                    highest = Some(i);
                    remaining = cap.map(|c| energy(c.saturating_sub(total)));
                }
                bands.push(Band {
                    index: i,
                    up_to: tier.up_to.clone(),
                    rate: tier.rate.clone(),
                    kwh: energy(amount),
                    cost: String::new(),
                    reached: amount > 0,
                });
                if let Some(c) = cap {
                    previous = c;
                }
            }
            let costs = allocate(round_div(raw, CENT_DIVISOR), &raw_bands);
            for (band, cents) in bands.iter_mut().zip(costs) {
                band.cost = money(cents);
            }
            if total == 0 {
                remaining = tiers[0].up_to.clone();
            }
            (raw, energies.clone())
        }
    };
    let cents = round_div(raw, CENT_DIVISOR);
    let allocations = allocate(cents, &weights);
    Ok((
        Totals {
            daily_kwh: energy(total / u128::from(days)),
            monthly_kwh: energy(total),
            monthly_cost: money(cents),
            daily_cost: money(round_div(cents, u128::from(days))),
            annual_cost: money(cents * 12),
            highest_tier: highest,
            remaining_kwh: remaining,
            bands,
        },
        allocations,
    ))
}
pub fn calculate(input: Input, ctx: &dyn TaskContext) -> PluginResult<Report> {
    validate(&input)?;
    ctx.stage("calculate");
    let baseline: Vec<_> = input
        .appliances
        .iter()
        .map(|a| usage_energy(&a.usage, a.quantity, input.days))
        .collect::<PluginResult<_>>()?;
    let alternative: Vec<_> = input
        .appliances
        .iter()
        .map(|a| {
            usage_energy(
                a.alternative.as_ref().unwrap_or(&a.usage),
                a.quantity,
                input.days,
            )
        })
        .collect::<PluginResult<_>>()?;
    let mut bills = Vec::new();
    for (i, tariff) in input.tariffs.iter().enumerate() {
        check(ctx)?;
        let (base, base_costs) = bill(&tariff.pricing, &baseline, input.days)?;
        let (alt, alt_costs) = bill(&tariff.pricing, &alternative, input.days)?;
        let shares = allocate(10_000, &base_costs);
        let mut rows = Vec::new();
        for (j, a) in input.appliances.iter().enumerate() {
            rows.push(Row {
                id: a.id.clone(),
                name: a.name.clone(),
                baseline_kwh: energy(baseline[j].iter().sum()),
                alternative_kwh: energy(alternative[j].iter().sum()),
                baseline_cost: money(base_costs[j]),
                alternative_cost: money(alt_costs[j]),
                share_percent: fixed(shares[j], 2),
            });
        }
        let a: u128 = base_costs.iter().sum();
        let b: u128 = alt_costs.iter().sum();
        bills.push(Bill {
            tariff: tariff.clone(),
            baseline: base,
            alternative: alt,
            annual_saving: money(a.abs_diff(b) * 12),
            saving_direction: if a > b {
                "saving"
            } else if a < b {
                "increase"
            } else {
                "same"
            }
            .into(),
            rows,
        });
        ctx.progress((i + 1) as u64, input.tariffs.len() as u64);
    }
    check(ctx)?;
    Ok(Report { input, bills })
}
