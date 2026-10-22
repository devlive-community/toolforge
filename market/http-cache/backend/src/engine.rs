use crate::{
    headers::{Control, Headers, token, vary},
    model::*,
    timing,
};
use tf_plugin_api::{PluginResult, TaskContext};

fn condition(out: &mut Vec<Condition>, key: &str, status: &str, note: Note) {
    out.push(Condition {
        key: key.into(),
        status: status.into(),
        note,
    });
}
fn uncertain(control: &Control, shared: bool) -> bool {
    control.malformed
        || control.boundary
        || control.invalid.iter().any(|v| shared || v != "s-maxage")
}
fn validators(headers: &Headers, notes: &mut Vec<Note>) -> String {
    let mut result = Vec::new();
    if let Some(value) = headers.one("etag", notes) {
        let tag = value.strip_prefix("W/").unwrap_or(value);
        if tag.starts_with('"')
            && tag.ends_with('"')
            && tag.len() >= 2
            && tag.as_bytes()[1..tag.len() - 1]
                .iter()
                .all(|b| *b >= 0x21 && *b != b'"' && *b != 127)
        {
            result.push(format!("If-None-Match: {value}"));
        } else {
            notes.push(Note::new("validator_invalid").with("field", "etag"));
        }
    }
    if let Some(value) = headers.one("last-modified", notes) {
        if timing::legacy_date(value) {
            notes.push(Note::new("legacy_date").with("field", "last-modified"));
        }
        if timing::http_date(value).is_some() {
            result.push(format!("If-Modified-Since: {value}"));
        } else {
            notes.push(Note::new("validator_invalid").with("field", "last-modified"));
        }
    }
    result.join("\n")
}
pub fn analyze(input: &Scenario, ctx: &dyn TaskContext) -> PluginResult<CaseReport> {
    check(ctx)?;
    if input.name.trim().is_empty()
        || input.name.chars().count() > 80
        || input.name.chars().any(char::is_control)
        || !token(&input.method)
        || input.method.len() > 32
        || input.status.len() != 3
        || !input.status.bytes().all(|b| b.is_ascii_digit())
        || !input
            .status
            .parse::<u16>()
            .is_ok_and(|n| (100..=599).contains(&n))
    {
        return Err(error("scenario"));
    }
    let mut notes = Vec::new();
    let original = Headers::parse(&input.request_headers, "request", &mut notes)?;
    let response = Headers::parse(&input.response_headers, "response", &mut notes)?;
    let new = Headers::parse(
        input
            .new_request_headers
            .as_deref()
            .unwrap_or(&input.request_headers),
        "presented",
        &mut notes,
    )?;
    ctx.stage("directives");
    let req = Control::parse(&original, "request", false, &mut notes);
    let res = Control::parse(&response, "response", true, &mut notes);
    let presented = Control::parse(&new, "presented", false, &mut notes);
    if [&req, &res, &presented]
        .iter()
        .any(|c| c.entries.len() > 256)
    {
        return Err(error("directives_limit"));
    }
    let shared = input.role == Role::Shared;
    if !shared && res.has("s-maxage") {
        notes.push(Note::new("smaxage_private"));
    }
    if res.has("public") && res.has("private") {
        notes.push(Note::new("public_private"));
    }
    if res.has("no-store") && (res.has("public") || res.has("max-age") || res.has("s-maxage")) {
        notes.push(Note::new("store_conflict"));
    }
    ctx.stage("age");
    let timing = timing::calculate(input, &response, &res, &mut notes)?;
    ctx.stage("variants");
    let (vary_status, vary_rows) = vary(&response, &original, &new, &mut notes);
    let conditional = validators(&response, &mut notes);
    let mut conditions = Vec::new();
    let scope = input.method == "GET"
        && input.status == "200"
        && !original.has("range")
        && !new.has("range")
        && !response.has("content-range");
    condition(
        &mut conditions,
        "scope",
        if scope { "pass" } else { "unknown" },
        Note::new(if scope {
            "scope_supported"
        } else {
            "scope_boundary"
        }),
    );
    condition(
        &mut conditions,
        "identity",
        "info",
        Note::new("same_uri_complete"),
    );
    let overrides = scope && res.flag("must-understand") && res.flag("no-store");
    if overrides {
        notes.push(Note::new("must_understand_override"));
    }
    let no_store = req.flag("no-store") || res.flag("no-store") && !overrides;
    let authorization_allowed =
        res.flag("public") || res.flag("must-revalidate") || res.number("s-maxage").is_some();
    let auth_blocked = shared && original.has("authorization") && !authorization_allowed;
    let private_blocked = shared && res.flag("private");
    let storage = if !scope {
        "unknown"
    } else if no_store || auth_blocked || private_blocked {
        "blocked"
    } else if original.malformed
        || response.malformed
        || req.malformed
        || res.malformed
        || req.invalid.contains("no-store")
        || res
            .invalid
            .iter()
            .any(|n| ["no-store", "private", "public", "must-understand"].contains(&n.as_str()))
        || shared && res.qualified("private")
    {
        "unknown"
    } else {
        "allowed"
    };
    condition(
        &mut conditions,
        "storage",
        if storage == "allowed" {
            "pass"
        } else if storage == "blocked" {
            "fail"
        } else {
            "unknown"
        },
        Note::new(if no_store {
            "no_store"
        } else if private_blocked {
            "private_shared"
        } else if auth_blocked {
            "authorization_blocked"
        } else if storage == "unknown" {
            "storage_uncertain"
        } else {
            "storage_allowed"
        }),
    );
    if shared && original.has("authorization") && authorization_allowed {
        condition(
            &mut conditions,
            "authorization",
            "pass",
            Note::new("authorization_allowed"),
        );
    }
    if response.has("set-cookie") || original.has("cookie") || new.has("cookie") {
        notes.push(Note::new("cookie_no_implicit_ban"));
    }
    if new.has("pragma") || response.has("pragma") || original.has("pragma") {
        notes.push(Note::new("pragma_boundary"));
    }
    let precondition = [
        "if-match",
        "if-none-match",
        "if-unmodified-since",
        "if-modified-since",
        "if-range",
    ]
    .iter()
    .any(|h| new.has(h));
    if precondition {
        notes.push(Note::new("precondition_boundary"));
    }
    let connection = [&original, &new, &response]
        .iter()
        .any(|h| h.has("connection"));
    if connection {
        notes.push(Note::new("connection_boundary"));
    }
    if input.new_request_headers.is_none() {
        notes.push(Note::new("same_request_assumed"));
    }
    if presented.flag("no-store") {
        notes.push(Note::new("presented_no_store"));
    }
    condition(
        &mut conditions,
        "vary",
        if vary_status == "match" {
            "pass"
        } else if vary_status == "different" {
            "fail"
        } else {
            "unknown"
        },
        Note::new(match vary_status.as_str() {
            "match" => "vary_match",
            "different" => "vary_different",
            _ => "vary_invalid",
        }),
    );
    condition(
        &mut conditions,
        "freshness",
        match timing.freshness.as_str() {
            "fresh" => "pass",
            "stale" => "fail",
            _ => "unknown",
        },
        Note::new(match timing.freshness.as_str() {
            "fresh" => "is_fresh",
            "stale" => "is_stale",
            _ => "freshness_unknown",
        }),
    );
    let forced = res.flag("no-cache") || presented.flag("no-cache");
    if forced {
        condition(&mut conditions, "no-cache", "fail", Note::new("no_cache"));
    }
    let mut requested = false;
    let mut request_unknown = false;
    if let Some(max) = presented.number("max-age") {
        let ok = timing.current_age.map(|age| age <= max);
        requested |= ok == Some(false);
        request_unknown |= ok.is_none();
        condition(
            &mut conditions,
            "request-max-age",
            match ok {
                Some(true) => "pass",
                Some(false) => "fail",
                None => "unknown",
            },
            Note::new("request_max_age").with("seconds", max),
        );
    }
    if let Some(min) = presented.number("min-fresh") {
        let ok = timing
            .current_age
            .zip(timing.lifetime)
            .map(|(age, l)| l >= age + min);
        requested |= ok == Some(false);
        request_unknown |= ok.is_none();
        condition(
            &mut conditions,
            "min-fresh",
            match ok {
                Some(true) => "pass",
                Some(false) => "fail",
                None => "unknown",
            },
            Note::new("request_min_fresh").with("seconds", min),
        );
    }
    if timing.freshness == "stale"
        && (res.flag("must-revalidate")
            || shared && (res.flag("proxy-revalidate") || res.number("s-maxage").is_some()))
    {
        condition(
            &mut conditions,
            "stale-validation",
            "fail",
            Note::new("stale_must_validate"),
        );
    }
    let uncertainty = original.malformed
        || response.malformed
        || new.malformed
        || uncertain(&res, shared)
        || uncertain(&presented, shared)
        || vary_status == "unknown"
        || precondition
        || connection
        || new.has("pragma")
        || response.has("pragma")
        || notes
            .iter()
            .any(|n| n.code == "delta_clamped" && n.params["context"] == "presented");
    let reuse = if storage == "blocked" {
        "blocked"
    } else if storage == "unknown" || !scope {
        "unknown"
    } else if vary_status == "different" {
        "variant"
    } else if shared && new.has("authorization") && !authorization_allowed {
        notes.push(Note::new("presented_authorization"));
        "blocked"
    } else if forced {
        "validate"
    } else if uncertainty {
        "unknown"
    } else if requested || timing.freshness == "stale" {
        "validate"
    } else if request_unknown || timing.freshness == "unknown" {
        "unknown"
    } else {
        "candidate"
    };
    let only_if_cached_unsatisfied =
        presented.flag("only-if-cached") && matches!(reuse, "validate" | "variant" | "blocked");
    if only_if_cached_unsatisfied {
        notes.push(Note::new("only_if_cached_unsatisfied"));
    }
    if conditional.is_empty() {
        notes.push(Note::new("no_validators"));
    }
    let mut directives = req.entries;
    directives.extend(res.entries);
    directives.extend(presented.entries);
    if notes.len() > 1024 {
        return Err(error("directives_limit"));
    }
    check(ctx)?;
    Ok(CaseReport {
        summary: Summary {
            name: input.name.clone(),
            role: input.role,
            storage: storage.into(),
            reuse: reuse.into(),
            freshness: timing.freshness.clone(),
            lifetime: timing.lifetime,
            current_age: timing.current_age,
            remaining: timing.remaining,
            lifetime_source: timing.lifetime_source.clone(),
            diagnostics: notes.len(),
        },
        method: input.method.clone(),
        status: input.status.clone(),
        request_headers: original.redacted(),
        response_headers: response.redacted(),
        new_request_headers: new.redacted(),
        same_request_assumed: input.new_request_headers.is_none(),
        timing,
        conditions,
        diagnostics: notes,
        directives,
        vary: vary_rows,
        conditional,
        only_if_cached_unsatisfied,
    })
}
