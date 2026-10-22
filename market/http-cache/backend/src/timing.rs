use crate::{
    headers::{Control, Headers, delta},
    model::*,
};
use std::time::UNIX_EPOCH;
use tf_plugin_api::PluginResult;

pub fn legacy_date(text: &str) -> bool {
    text.split_once(',')
        .is_some_and(|(_, date)| date.trim().as_bytes().get(2) == Some(&b'-'))
}
pub fn http_date(text: &str) -> Option<i64> {
    // Two-digit RFC 850 years require a reference-dependent 50-year window.
    // This version keeps them explicitly unknown instead of using a fixed pivot.
    if legacy_date(text) {
        return None;
    }
    // HTTP field tokens are case-insensitive for cache date parsing. Normalize
    // only the standardized alphabetic date words, never arbitrary values.
    let mut text = text.to_owned();
    for word in [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
        "Mon",
        "Tue",
        "Wed",
        "Thu",
        "Fri",
        "Sat",
        "Sun",
        "Jan",
        "Feb",
        "Mar",
        "Apr",
        "May",
        "Jun",
        "Jul",
        "Aug",
        "Sep",
        "Oct",
        "Nov",
        "Dec",
        "GMT",
    ] {
        let lower = text.to_ascii_lowercase();
        let needle = word.to_ascii_lowercase();
        if let Some(at) = lower.find(&needle) {
            text.replace_range(at..at + word.len(), word);
        }
    }
    httpdate::parse_http_date(&text)
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_secs()).ok())
}
fn instant(text: &str, field: &str, notes: &mut Vec<Note>) -> PluginResult<Option<i64>> {
    if text.len() > 64 {
        return Err(error("time").with("field", field));
    }
    if text.trim().is_empty() {
        notes.push(Note::new("time_missing").with("field", field));
        return Ok(None);
    }
    let text = text.trim();
    // Explicit offsets and whole seconds make every displayed step reproducible.
    if text.len() > 40
        || text.as_bytes().get(10) != Some(&b'T')
        || text.as_bytes().get(13) != Some(&b':')
        || text.as_bytes().get(16) != Some(&b':')
        || text.get(17..19) == Some("60")
        || text.contains(['[', ']'])
    {
        return Err(error("time").with("field", field));
    }
    let ts = text
        .parse::<jiff::Timestamp>()
        .map_err(|_| error("time").with("field", field))?;
    if ts.subsec_nanosecond() != 0 || ts.as_second() < 0 {
        return Err(error("time").with("field", field));
    }
    Ok(Some(ts.as_second()))
}
pub fn calculate(
    input: &Scenario,
    headers: &Headers,
    control: &Control,
    notes: &mut Vec<Note>,
) -> PluginResult<Timing> {
    let mut t = Timing {
        request_time: instant(&input.request_time, "requestTime", notes)?,
        response_time: instant(&input.response_time, "responseTime", notes)?,
        now: instant(&input.now, "now", notes)?,
        ..Timing::default()
    };
    let date_value = headers.one("date", notes);
    if date_value.is_some_and(legacy_date) {
        notes.push(Note::new("legacy_date").with("field", "date"));
    }
    t.date = date_value.and_then(http_date);
    if t.date.is_none() {
        notes.push(Note::new(if headers.has("date") {
            "date_invalid"
        } else {
            "date_missing"
        }));
    }
    let ages = headers.values("age");
    if let Some(first) = ages.first() {
        if ages.len() > 1 || first.contains(',') {
            notes.push(Note::new("age_first"));
        }
        t.age_value = delta(
            first.split(',').next().unwrap_or_default().trim(),
            notes,
            "age",
        )
        .unwrap_or_else(|| {
            notes.push(Note::new("age_invalid"));
            0
        });
    } else {
        notes.push(Note::new("age_absent"));
    }
    let chronological = !matches!((t.request_time,t.response_time),(Some(a),Some(b)) if b<a)
        && !matches!((t.response_time,t.now),(Some(a),Some(b)) if b<a);
    if !chronological {
        notes.push(Note::new("time_order"));
    }
    if chronological {
        t.apparent_age = t
            .response_time
            .zip(t.date)
            .map(|(r, d)| (r - d).max(0) as u64);
        if matches!((t.response_time,t.date),(Some(r),Some(d)) if d>r) {
            notes.push(Note::new("date_future"));
        }
        t.response_delay = t
            .response_time
            .zip(t.request_time)
            .map(|(r, q)| (r - q) as u64);
        t.corrected_age_value = t.response_delay.map(|d| t.age_value + d);
        t.corrected_initial_age = t
            .apparent_age
            .zip(t.corrected_age_value)
            .map(|(a, b)| a.max(b));
        t.resident_time = t.now.zip(t.response_time).map(|(n, r)| (n - r) as u64);
        t.current_age = t
            .corrected_initial_age
            .zip(t.resident_time)
            .map(|(a, b)| a + b);
    }
    let numeric = if input.role == Role::Shared && control.has("s-maxage") {
        Some("s-maxage")
    } else if control.has("max-age") {
        Some("max-age")
    } else {
        None
    };
    if let Some(field) = numeric {
        t.lifetime_source = field.into();
        t.lifetime = control.number(field);
        if t.lifetime.is_none() {
            t.lifetime = Some(0);
            t.lifetime_source = "invalid".into();
            notes.push(Note::new("freshness_invalid").with("field", field));
        }
        if headers.has("expires") {
            notes.push(Note::new("expires_ignored").with("field", field));
        }
    } else if headers.has("expires") {
        t.lifetime_source = "expires".into();
        let expires_value = headers.one("expires", notes);
        if expires_value.is_some_and(legacy_date) {
            notes.push(Note::new("legacy_date").with("field", "expires"));
        }
        let expires = expires_value.and_then(http_date);
        if expires_value.is_some_and(legacy_date) {
            t.lifetime = None;
            t.lifetime_source = "unsupported".into();
        } else if let Some(exp) = expires {
            let base = if !headers.has("date") {
                notes.push(Note::new("expires_received_date"));
                t.response_time
            } else {
                t.date
            };
            t.lifetime = base.map(|date| (exp - date).max(0) as u64);
        } else {
            t.lifetime = Some(0);
            notes.push(Note::new("expires_invalid"));
        }
    } else {
        t.lifetime_source = "none".into();
        notes.push(Note::new("no_heuristic"));
    }
    t.freshness = match (t.lifetime, t.current_age) {
        (Some(l), Some(age)) if l > age => "fresh",
        (Some(_), Some(_)) | (Some(0), _) => "stale",
        _ => "unknown",
    }
    .into();
    if notes
        .iter()
        .any(|n| n.code == "delta_clamped" && n.params["field"] == "age")
    {
        t.current_age = None;
    }
    if notes.iter().any(|n| {
        n.code == "delta_clamped"
            && n.params["context"] == "response"
            && n.params["field"].as_str() == Some(t.lifetime_source.as_str())
    }) {
        t.lifetime = None;
    }
    if t.current_age.is_none() && t.lifetime != Some(0) || t.lifetime.is_none() {
        t.freshness = "unknown".into();
    }
    t.remaining = t
        .lifetime
        .zip(t.current_age)
        .map(|(l, a)| l as i64 - a as i64);
    Ok(t)
}
