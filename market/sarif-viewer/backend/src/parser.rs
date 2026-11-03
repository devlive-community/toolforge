use crate::model::*;
use serde_json::Value;
use std::collections::HashMap;
use tf_plugin_api::{PluginResult, TaskContext};

/// Bounded depth walk after deserialization; serde_json caps nesting at 128
/// while this tool promises 64, so depth is verified explicitly afterwards.
fn walk(value: &Value, depth: usize, ctx: &dyn TaskContext) -> PluginResult<()> {
    if depth > JSON_DEPTH_LIMIT {
        return Err(error("depth"));
    }
    match value {
        Value::Array(items) => {
            for item in items {
                check(ctx)?;
                walk(item, depth + 1, ctx)?;
            }
        }
        Value::Object(map) => {
            for (_, item) in map {
                check(ctx)?;
                walk(item, depth + 1, ctx)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn bounded(s: &str, limit: usize) -> PluginResult<String> {
    if s.len() > limit {
        Err(error("string"))
    } else {
        Ok(s.to_owned())
    }
}

/// SARIF `message`/`description` nodes carry text in `.text` (or `.markdown`).
fn node_text(value: &Value) -> Option<&str> {
    if value.is_string() {
        value.as_str()
    } else {
        value.get("text").and_then(Value::as_str)
    }
}

fn collect_message_strings(value: Option<&Value>) -> PluginResult<HashMap<String, String>> {
    let mut output = HashMap::new();
    if let Some(map) = value.and_then(Value::as_object) {
        if map.len() > 128 {
            return Err(error("entries"));
        }
        for (key, entry) in map {
            if let Some(text) = node_text(entry) {
                output.insert(key.clone(), bounded(text, STRING_LIMIT)?);
            }
        }
    }
    Ok(output)
}

fn region(
    value: &Value,
    report: &mut Report,
    run: usize,
    result: Option<usize>,
) -> PluginResult<Option<Region>> {
    let map = match value.as_object() {
        Some(map) => map,
        None => return Ok(None),
    };
    let mut parsed = Region {
        start_line: None,
        start_column: None,
        end_line: None,
        end_column: None,
    };
    let mut invalid = false;
    for (key, target) in [
        ("startLine", &mut parsed.start_line),
        ("startColumn", &mut parsed.start_column),
        ("endLine", &mut parsed.end_line),
        ("endColumn", &mut parsed.end_column),
    ] {
        match map.get(key).and_then(Value::as_u64) {
            Some(raw) if (1..=(1u64 << 31)).contains(&raw) => *target = Some(raw),
            Some(_) => invalid = true,
            None => {}
        }
    }
    if invalid {
        report.diagnostic(run, result, "region", "coordinate out of range")?;
    }
    Ok(Some(parsed))
}

struct RunScope {
    artifact_base: usize,
    artifact_count: usize,
    rules: Vec<usize>,
}

/// Resolve a result's rule by explicit `ruleIndex` first, falling back to a
/// `ruleId` lookup inside the driver rule table. Unresolvable references keep
/// the raw ruleId for display.
fn resolve_rule(
    report: &mut Report,
    run: usize,
    result: usize,
    scope: &RunScope,
    rule_id: &str,
    rule_index: Option<u64>,
) -> PluginResult<Option<usize>> {
    if let Some(index) = rule_index {
        if (index as usize) < scope.rules.len() {
            let rule = scope.rules[index as usize];
            if report.rules[rule].rule_id != rule_id {
                report.diagnostic(
                    run,
                    Some(result),
                    "mismatch",
                    format!("ruleIndex {index} declares {}", report.rules[rule].rule_id),
                )?;
            }
            return Ok(Some(rule));
        }
        report.diagnostic(
            run,
            Some(result),
            "ruleIndex",
            format!("index {index} out of range"),
        )?;
        return Ok(None);
    }
    Ok(scope
        .rules
        .iter()
        .find(|&&rule| report.rules[rule].rule_id == rule_id)
        .copied())
}

/// Message precedence: inline `text`, then `markdown`, then the rule's
/// `messageStrings[id]` template with `{0}`-style arguments interpolated.
fn message_text(
    report: &mut Report,
    run: usize,
    rule: Option<usize>,
    message: &Value,
) -> PluginResult<String> {
    if let Some(text) = message.get("text").and_then(Value::as_str) {
        return bounded(text, STRING_LIMIT);
    }
    if let Some(markdown) = message.get("markdown").and_then(Value::as_str) {
        return bounded(markdown, STRING_LIMIT);
    }
    if let Some(id) = message.get("id").and_then(Value::as_str) {
        let template = rule
            .and_then(|rule| report.rules.get(rule))
            .and_then(|row| row.messages.get(id))
            .cloned();
        if let Some(template) = template {
            let mut output = template;
            if let Some(arguments) = message.get("arguments").and_then(Value::as_array) {
                for (index, argument) in arguments.iter().enumerate().take(16) {
                    let placeholder = format!("{{{index}}}");
                    let value = argument.as_str().unwrap_or("").to_owned();
                    output = output.replace(&placeholder, &value);
                }
            }
            return bounded(&output, STRING_LIMIT);
        }
        report.diagnostic(run, None, "message", format!("unknown message id {id}"))?;
        return bounded(id, STRING_LIMIT);
    }
    report.diagnostic(run, None, "message", "result has no message text")?;
    Ok(String::new())
}

fn parse_location(
    report: &mut Report,
    run: usize,
    result: usize,
    scope: &RunScope,
    value: &Value,
    kind: &str,
) -> PluginResult<usize> {
    let id = report.locations.len();
    let mut artifact = None;
    let mut uri = None;
    let mut uri_base_id = None;
    let mut region_out = None;
    let mut snippet = None;
    if let Some(physical) = value.get("physicalLocation") {
        if let Some(artifact_location) = physical.get("artifactLocation") {
            uri_base_id = artifact_location
                .get("uriBaseId")
                .and_then(Value::as_str)
                .map(str::to_owned);
            if let Some(raw) = artifact_location.get("uri").and_then(Value::as_str) {
                uri = Some(bounded(raw, STRING_LIMIT)?);
            }
            if let Some(index) = artifact_location.get("index").and_then(Value::as_u64) {
                if index < scope.artifact_count as u64 {
                    artifact = Some(scope.artifact_base + index as usize);
                } else {
                    report.diagnostic(run, Some(result), "artifact", format!("index {index}"))?;
                }
            }
        }
        region_out = physical
            .get("region")
            .map(|r| region(r, report, run, Some(result)))
            .transpose()?
            .flatten();
        snippet = physical
            .get("region")
            .and_then(|r| r.get("snippet"))
            .and_then(node_text)
            .map(|s| bounded(s, SNIPPET_LIMIT))
            .transpose()?;
    }
    let logical = value
        .get("logicalLocations")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("name"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let message = value
        .get("message")
        .and_then(node_text)
        .map(|s| bounded(s, STRING_LIMIT))
        .transpose()?;
    report.locations.push(LocationRow {
        id,
        run,
        result,
        kind: kind.into(),
        artifact,
        uri,
        uri_base_id,
        region: region_out,
        snippet,
        logical,
        message,
        order: None,
        nesting: None,
    });
    report.budget()?;
    Ok(id)
}

fn parse_result(
    report: &mut Report,
    run: usize,
    scope: &RunScope,
    value: &Value,
    ctx: &dyn TaskContext,
) -> PluginResult<()> {
    let id = report.results.len();
    let rule_id = value
        .get("ruleId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    if value.get("ruleId").is_some() && rule_id.is_empty() {
        report.diagnostic(run, Some(id), "ruleId", "empty ruleId")?;
    }
    let rule_index = value.get("ruleIndex").and_then(Value::as_u64);
    let rule = resolve_rule(report, run, id, scope, &rule_id, rule_index)?;
    // Level precedence: explicit result.level > rule default level > kind.
    let kind = match value.get("kind").and_then(Value::as_str) {
        Some(kind @ ("fail" | "pass" | "review" | "open" | "notApplicable" | "informational")) => {
            kind.to_owned()
        }
        Some(other) => {
            report.diagnostic(run, Some(id), "kind", other)?;
            "unknown".to_owned()
        }
        None => "fail".to_owned(),
    };
    let explicit = value.get("level").and_then(Value::as_str);
    let mut level = explicit.and_then(level_of);
    if let Some(raw) = explicit
        && level.is_none()
    {
        report.diagnostic(run, Some(id), "level", raw)?;
    }
    if level.is_none() {
        level = rule.and_then(|rule| report.rules[rule].default_level);
    }
    let level = level.unwrap_or(if kind == "fail" {
        Level::Error
    } else {
        Level::None
    });
    let message = message_text(
        report,
        run,
        rule,
        &value.get("message").cloned().unwrap_or(Value::Null),
    )?;
    let baseline_state = value
        .get("baselineState")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let suppressions: Vec<Suppression> = value
        .get("suppressions")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .take(SUPPRESSIONS_PER_RESULT)
                .map(|item| {
                    Ok(Suppression {
                        kind: item
                            .get("kind")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown")
                            .to_owned(),
                        status: item
                            .get("status")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        justification: item
                            .get("justification")
                            .and_then(Value::as_str)
                            .map(|s| bounded(s, STRING_LIMIT))
                            .transpose()?,
                    })
                })
                .collect::<PluginResult<Vec<_>>>()
        })
        .transpose()?
        .unwrap_or_default();
    let mut row = ResultRow {
        id,
        run,
        rule,
        rule_id,
        level,
        kind,
        message,
        baseline_state,
        rank: value.get("rank").and_then(Value::as_f64),
        suppressed: !suppressions.is_empty(),
        suppressions,
        occurrence_count: value.get("occurrenceCount").and_then(Value::as_u64),
        locations: Vec::new(),
        related: Vec::new(),
        flows: Vec::new(),
    };
    if let Some(locations) = value.get("locations").and_then(Value::as_array) {
        if locations.len() > LOCATIONS_PER_RESULT {
            report.diagnostic(run, Some(id), "locations", "truncated")?;
        }
        for location in locations.iter().take(LOCATIONS_PER_RESULT) {
            check(ctx)?;
            let location = parse_location(report, run, id, scope, location, "physical")?;
            row.locations.push(location);
        }
    }
    if let Some(related) = value.get("relatedLocations").and_then(Value::as_array) {
        if related.len() > RELATED_PER_RESULT {
            report.diagnostic(run, Some(id), "related", "truncated")?;
        }
        for location in related.iter().take(RELATED_PER_RESULT) {
            check(ctx)?;
            let location = parse_location(report, run, id, scope, location, "related")?;
            row.related.push(location);
        }
    }
    if let Some(flows) = value.get("codeFlows").and_then(Value::as_array) {
        if flows.len() > FLOWS_PER_RESULT {
            report.diagnostic(run, Some(id), "codeFlows", "truncated")?;
        }
        for flow in flows.iter().take(FLOWS_PER_RESULT) {
            check(ctx)?;
            let flow_id = report.flows.len();
            let mut steps = Vec::new();
            if let Some(threads) = flow.get("threadFlows").and_then(Value::as_array) {
                if threads.len() > THREADS_PER_FLOW {
                    report.diagnostic(run, Some(id), "threads", "truncated")?;
                }
                for thread in threads.iter().take(THREADS_PER_FLOW) {
                    if let Some(thread_locations) =
                        thread.get("locations").and_then(Value::as_array)
                    {
                        let remaining = STEPS_PER_FLOW.saturating_sub(steps.len());
                        for step in thread_locations.iter().take(remaining) {
                            check(ctx)?;
                            let message =
                                step.get("message").and_then(node_text).map(str::to_owned);
                            let location = parse_location(report, run, id, scope, step, "step")?;
                            if let Some(message) = message {
                                report.locations[location].message =
                                    Some(bounded(&message, STRING_LIMIT)?);
                            }
                            report.locations[location].order =
                                step.get("executionOrder").and_then(Value::as_u64);
                            report.locations[location].nesting =
                                step.get("nestingLevel").and_then(Value::as_u64);
                            steps.push(location);
                        }
                    }
                }
            }
            report.flows.push(FlowRow {
                id: flow_id,
                run,
                result: id,
                message: flow.get("message").and_then(node_text).map(str::to_owned),
                steps,
            });
            row.flows.push(flow_id);
        }
    }
    report.counts.add_result(&row);
    report.runs[run].result_count += 1;
    report.budget()?;
    report.results.push(row);
    Ok(())
}

fn parse_run(
    report: &mut Report,
    run: usize,
    file: usize,
    value: &Value,
    ctx: &dyn TaskContext,
) -> PluginResult<()> {
    check(ctx)?;
    let tool = value.get("tool").and_then(|tool| tool.get("driver"));
    let driver = tool.cloned().unwrap_or(Value::Null);
    let tool = driver
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    let column_kind = match value.get("columnKind").and_then(Value::as_str) {
        Some(kind @ ("utf16CodeUnits" | "unicodeCodePoints")) => kind.to_owned(),
        Some(other) => {
            report.diagnostic(run, None, "columnKind", other)?;
            "utf16CodeUnits".to_owned()
        }
        None => "utf16CodeUnits".to_owned(),
    };
    let external_properties = value.get("externalPropertyFileReferences").is_some();
    if external_properties {
        report.diagnostic(
            run,
            None,
            "external",
            "external property files are not supported",
        )?;
    }
    let invocation_success = value
        .get("invocations")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("executionSuccessful"))
        .and_then(Value::as_bool);
    let automation_id = value
        .get("automationDetails")
        .and_then(|details| details.get("id"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let run_id = report.runs.len();
    report.runs.push(RunRow {
        id: run_id,
        file,
        tool,
        version: driver
            .get("version")
            .and_then(Value::as_str)
            .map(str::to_owned),
        column_kind,
        artifact_count: 0,
        rule_count: 0,
        result_count: 0,
        extension_count: 0,
        external_properties,
        invocation_success,
        automation_id,
    });
    report.budget()?;

    // Rules declared by the driver only; extension components are flagged and
    // their rules stay referenced by raw ruleId.
    let mut scope = RunScope {
        artifact_base: report.artifacts.len(),
        artifact_count: 0,
        rules: Vec::new(),
    };
    if let Some(rules) = driver.get("rules").and_then(Value::as_array) {
        if rules.len() > RULES_PER_RUN {
            return Err(error("entries"));
        }
        for rule in rules {
            check(ctx)?;
            let rule_id = rule.get("id").and_then(Value::as_str).unwrap_or("");
            let rule_id = bounded(rule_id, STRING_LIMIT)?;
            let index = report.rules.len();
            report.rules.push(RuleRow {
                id: index,
                run,
                rule_id,
                messages: collect_message_strings(rule.get("messageStrings"))?,
                name: rule.get("name").and_then(Value::as_str).map(str::to_owned),
                short_description: rule
                    .get("shortDescription")
                    .and_then(node_text)
                    .map(str::to_owned),
                default_level: rule
                    .get("defaultConfiguration")
                    .and_then(|configuration| configuration.get("level"))
                    .and_then(Value::as_str)
                    .and_then(level_of),
                help_uri: rule
                    .get("helpUri")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            });
            scope.rules.push(index);
            report.budget()?;
        }
    }
    if let Some(extensions) = value
        .get("tool")
        .and_then(|tool| tool.get("extensions"))
        .and_then(Value::as_array)
    {
        report.runs[run].extension_count = extensions.len();
        report.diagnostic(
            run,
            None,
            "extensions",
            format!("{} extension component(s)", extensions.len()),
        )?;
    }
    if let Some(bases) = value.get("originalUriBaseIds").and_then(Value::as_object) {
        if bases.len() > BASES_PER_RUN {
            return Err(error("entries"));
        }
        for (key, base) in bases {
            check(ctx)?;
            let uri = base.get("uri").and_then(Value::as_str).unwrap_or("");
            report.bases.push(BaseRow {
                run,
                key: bounded(key, STRING_LIMIT)?,
                uri: bounded(uri, STRING_LIMIT)?,
            });
            report.budget()?;
        }
    }
    if let Some(artifacts) = value.get("artifacts").and_then(Value::as_array) {
        if artifacts.len() > ARTIFACTS_PER_RUN {
            return Err(error("entries"));
        }
        for artifact in artifacts {
            check(ctx)?;
            let location = artifact.get("location");
            let uri = location
                .and_then(|l| l.get("uri"))
                .and_then(Value::as_str)
                .unwrap_or("");
            report.artifacts.push(ArtifactRow {
                id: report.artifacts.len(),
                run,
                uri: bounded(uri, STRING_LIMIT)?,
                uri_base_id: location
                    .and_then(|l| l.get("uriBaseId"))
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                language: artifact
                    .get("sourceLanguage")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                roles: artifact
                    .get("roles")
                    .and_then(Value::as_array)
                    .map(|roles| {
                        roles
                            .iter()
                            .filter_map(Value::as_str)
                            .take(8)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default(),
            });
            scope.artifact_count += 1;
            report.budget()?;
        }
    }
    if let Some(results) = value.get("results").and_then(Value::as_array) {
        if results.len() > RESULTS_LIMIT {
            return Err(error("entries"));
        }
        for result in results {
            check(ctx)?;
            parse_result(report, run, &scope, result, ctx)?;
        }
    }
    report.runs[run].artifact_count = scope.artifact_count;
    report.runs[run].rule_count = scope.rules.len();
    Ok(())
}

pub fn parse(
    report: &mut Report,
    path: String,
    text: &str,
    ctx: &dyn TaskContext,
) -> PluginResult<()> {
    if text.len() > FILE_LIMIT {
        return Err(error("size"));
    }
    let root: Value =
        serde_json::from_str(text).map_err(|e| error("json").with("detail", e.to_string()))?;
    if !root.is_object() {
        return Err(error("root"));
    }
    walk(&root, 0, ctx)?;
    let runs = root
        .get("runs")
        .and_then(Value::as_array)
        .ok_or_else(|| error("runs"))?;
    if runs.is_empty() || runs.len() > RUNS_LIMIT {
        return Err(error("runs"));
    }
    match root.get("version").and_then(Value::as_str) {
        Some(version) if version.starts_with("2.") => {}
        Some(other) => return Err(error("version").with("found", other)),
        None => report.diagnostic(0, None, "version", "missing version")?,
    }
    let file = report.files.len();
    report.files.push(FileRow {
        id: file,
        name: path.rsplit(['/', '\\']).next().unwrap_or(&path).to_owned(),
        path,
    });
    for (index, run) in runs.iter().enumerate() {
        parse_run(report, index, file, run, ctx)?;
    }
    Ok(())
}

fn level_of(raw: &str) -> Option<Level> {
    match raw {
        "error" => Some(Level::Error),
        "warning" => Some(Level::Warning),
        "note" => Some(Level::Note),
        "none" => Some(Level::None),
        _ => None,
    }
}

#[cfg(test)]
#[path = "parser_test.rs"]
mod tests;
