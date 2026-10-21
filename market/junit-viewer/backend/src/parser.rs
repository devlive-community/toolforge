use crate::model::*;
use quick_xml::{Reader, events::Event};
use roxmltree::Node;
use tf_plugin_api::{PluginResult, TaskContext};

/// Check work budgets while streaming, before constructing the XML DOM.
fn preflight(text: &str, ctx: &dyn TaskContext) -> PluginResult<()> {
    if text.len() > FILE_LIMIT {
        return Err(error("size"));
    }
    let mut reader = Reader::from_str(text);
    let mut depth = 0usize;
    let mut nodes = 0usize;
    loop {
        check(ctx)?;
        let event = reader
            .read_event()
            .map_err(|e| error("xml").with("detail", e.to_string()))?;
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::DocType(_) => return Err(error("dtd")),
            Event::Decl(decl) => {
                if let Some(encoding) = decl.encoding() {
                    let encoding =
                        encoding.map_err(|e| error("xml").with("detail", e.to_string()))?;
                    if !encoding.eq_ignore_ascii_case("UTF-8")
                        && !encoding.eq_ignore_ascii_case("UTF8")
                        && !(encoding.eq_ignore_ascii_case("US-ASCII") && text.is_ascii())
                    {
                        return Err(error("encoding"));
                    }
                }
            }
            Event::Start(tag) | Event::Empty(tag) => {
                for attr in tag.attributes() {
                    let attr = attr.map_err(|e| error("xml").with("detail", e.to_string()))?;
                    if attr.value.len() > 4096 {
                        return Err(error("attribute"));
                    }
                }
                nodes += 1;
                if depth >= 64 {
                    return Err(error("depth"));
                }
                // Empty tags have no matching End event.
                if !empty {
                    depth += 1;
                }
            }
            Event::End(_) => {
                depth = depth.saturating_sub(1);
            }
            Event::Eof => break,
            _ => {}
        }
        if depth > 64 {
            return Err(error("depth"));
        }
        if nodes > 200_000 {
            return Err(error("entries"));
        }
    }
    Ok(())
}

fn duration(
    node: Node<'_, '_>,
    report: &mut Report,
    source: usize,
    suite: usize,
    case: Option<usize>,
) -> PluginResult<Option<f64>> {
    match node.attribute("time") {
        None => Ok(None),
        Some(raw) => match raw.parse::<f64>() {
            Ok(t) if t.is_finite() && (0.0..=1e12).contains(&t) => Ok(Some(t)),
            _ => {
                report.diagnostic(source, Some(suite), case, "time", raw)?;
                Ok(None)
            }
        },
    }
}

fn content(
    node: Node<'_, '_>,
    report: &mut Report,
    source: usize,
    suite: usize,
    case: Option<usize>,
    ctx: &dyn TaskContext,
) -> PluginResult<usize> {
    let id = report.content.len();
    let mut text = String::new();
    for child in node.descendants() {
        check(ctx)?;
        if child.is_text() {
            text.push_str(child.text().unwrap_or(""));
        }
    }
    report.content.push(Content {
        id,
        source,
        suite,
        case,
        kind: node.tag_name().name().into(),
        message: node.attribute("message").map(str::to_owned),
        type_name: node.attribute("type").map(str::to_owned),
        text,
    });
    report.budget()?;
    Ok(id)
}

fn testcase(
    node: Node<'_, '_>,
    report: &mut Report,
    source: usize,
    suite: usize,
    ctx: &dyn TaskContext,
) -> PluginResult<()> {
    let id = report.cases.len();
    let mut statuses = Vec::new();
    let mut blocks = Vec::new();
    let mut unknown = false;
    for child in node.children().filter(Node::is_element) {
        check(ctx)?;
        let kind = child.tag_name().name();
        match kind {
            "failure" | "error" | "skipped" | "system-out" | "system-err" => {
                blocks.push(content(child, report, source, suite, Some(id), ctx)?);
                match kind {
                    "failure" => statuses.push(Status::Failure),
                    "error" => statuses.push(Status::Error),
                    "skipped" => statuses.push(Status::Skipped),
                    _ => {}
                }
            }
            "properties" => {}
            _ => {
                unknown = true;
                report.diagnostic(source, Some(suite), Some(id), "extension", kind)?;
                blocks.push(content(child, report, source, suite, Some(id), ctx)?);
            }
        }
    }
    for raw in [node.attribute("status"), node.attribute("result")]
        .into_iter()
        .flatten()
    {
        match raw.to_ascii_lowercase().as_str() {
            "run" | "completed" => {}
            "passed" | "success" => statuses.push(Status::Passed),
            "failed" | "failure" => statuses.push(Status::Failure),
            "error" => statuses.push(Status::Error),
            "skipped" | "notrun" | "disabled" => statuses.push(Status::Skipped),
            _ => {
                unknown = true;
                report.diagnostic(source, Some(suite), Some(id), "status", raw)?;
            }
        }
    }
    if statuses
        .first()
        .is_some_and(|first| statuses.iter().any(|s| s != first))
    {
        unknown = true;
        report.diagnostic(source, Some(suite), Some(id), "conflict", "")?;
    }
    if node.attribute("name").is_none() {
        unknown = true;
        report.diagnostic(source, Some(suite), Some(id), "name", "")?;
    }
    let seconds = duration(node, report, source, suite, Some(id))?;
    let case = Case {
        id,
        source,
        suite,
        name: node.attribute("name").unwrap_or("").into(),
        classname: node.attribute("classname").unwrap_or("").into(),
        status: if unknown {
            Status::Unknown
        } else {
            statuses.first().copied().unwrap_or(Status::Passed)
        },
        seconds,
        file: node.attribute("file").map(str::to_owned),
        line: node.attribute("line").map(str::to_owned),
        content: blocks,
    };
    report.suites[suite].counts.add_case(&case);
    report.cases.push(case);
    report.budget()
}

fn suite(
    node: Node<'_, '_>,
    report: &mut Report,
    source: usize,
    parent: Option<usize>,
    ctx: &dyn TaskContext,
) -> PluginResult<()> {
    check(ctx)?;
    let id = report.suites.len();
    let mut declared = Declared::default();
    for (key, target) in [
        ("tests", &mut declared.tests),
        ("failures", &mut declared.failures),
        ("errors", &mut declared.errors),
        ("skipped", &mut declared.skipped),
    ] {
        if let Some(raw) = node.attribute(key) {
            match raw.parse::<usize>() {
                Ok(value) => *target = Some(value),
                _ => {
                    report.diagnostic(source, Some(id), None, "declared", format!("{key}={raw}"))?
                }
            }
        }
    }
    declared.seconds = duration(node, report, source, id, None)?;
    report.suites.push(Suite {
        id,
        source,
        parent,
        name: node.attribute("name").unwrap_or("").into(),
        kind: node.tag_name().name().into(),
        declared,
        counts: Counts::default(),
        content: Vec::new(),
    });
    report.budget()?;
    for child in node.children().filter(Node::is_element) {
        check(ctx)?;
        match child.tag_name().name() {
            "testsuite" | "testsuites" => suite(child, report, source, Some(id), ctx)?,
            "testcase" if node.tag_name().name() == "testsuite" => {
                testcase(child, report, source, id, ctx)?
            }
            "system-out" | "system-err" => {
                let block = content(child, report, source, id, None, ctx)?;
                report.suites[id].content.push(block);
            }
            "properties" => {}
            other => report.diagnostic(source, Some(id), None, "extension", other)?,
        }
    }
    let s = &report.suites[id];
    let mut mismatches = Vec::new();
    for (key, declared, actual) in [
        ("tests", s.declared.tests, s.counts.tests),
        ("failures", s.declared.failures, s.counts.failures),
        ("errors", s.declared.errors, s.counts.errors),
        ("skipped", s.declared.skipped, s.counts.skipped),
    ] {
        if let Some(declared) = declared
            && declared != actual
        {
            mismatches.push(format!("{key}: {declared} / {actual}"));
        }
    }
    for mismatch in mismatches {
        report.diagnostic(source, Some(id), None, "counts", mismatch)?;
    }
    let counts = report.suites[id].counts.clone();
    match parent {
        Some(parent) => report.suites[parent].counts.add(&counts),
        None => {
            report.sources[source].counts.add(&counts);
            report.counts.add(&counts);
        }
    }
    Ok(())
}

pub fn parse(
    report: &mut Report,
    path: String,
    text: &str,
    ctx: &dyn TaskContext,
) -> PluginResult<()> {
    let text = text.trim_start_matches('\u{feff}');
    preflight(text, ctx)?;
    let doc = roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 250_000,
        },
    )
    .map_err(|e| error("xml").with("detail", e.to_string()))?;
    check(ctx)?;
    let root = doc.root_element();
    if !["testsuite", "testsuites"].contains(&root.tag_name().name()) {
        return Err(error("root"));
    }
    let source = report.sources.len();
    let name = path.rsplit(['/', '\\']).next().unwrap_or(&path).to_owned();
    report.sources.push(Source {
        id: source,
        path,
        name,
        counts: Counts::default(),
    });
    suite(root, report, source, None, ctx)
}

#[cfg(test)]
#[path = "parser_test.rs"]
mod tests;
