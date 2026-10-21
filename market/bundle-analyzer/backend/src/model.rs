use serde::{
    Deserialize, Serialize,
    de::{MapAccess, Visitor},
};
use std::collections::BTreeMap;
use tf_plugin_api::{PluginError, PluginResult, TaskContext, cancelled};

pub const FILE_LIMIT: usize = 16 * 1024 * 1024;
pub const SAFE: u64 = 9_007_199_254_740_991;
pub fn error(code: &str) -> PluginError {
    PluginError::new(format!("bundle.{code}"))
}
pub fn check(ctx: &dyn TaskContext) -> PluginResult<()> {
    if ctx.is_cancelled() {
        Err(cancelled())
    } else {
        Ok(())
    }
}
pub fn add(a: u64, b: u64) -> PluginResult<u64> {
    a.checked_add(b)
        .filter(|&n| n <= SAFE)
        .ok_or_else(|| error("bytes"))
}
fn unique<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<BTreeMap<String, T>, D::Error> {
    struct Unique<T>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> Visitor<'de> for Unique<T> {
        type Value = BTreeMap<String, T>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a map with unique paths")
        }
        fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
            let mut out = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<String, T>()? {
                if out.insert(key, value).is_some() {
                    return Err(serde::de::Error::custom("duplicate path"));
                }
            }
            Ok(out)
        }
    }
    d.deserialize_map(Unique(std::marker::PhantomData))
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Import {
    pub path: String,
    pub kind: String,
    #[serde(default)]
    pub external: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub bytes: u64,
    pub imports: Vec<Import>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contribution {
    pub bytes_in_output: u64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub bytes: u64,
    #[serde(deserialize_with = "unique")]
    pub inputs: BTreeMap<String, Contribution>,
    pub imports: Vec<Import>,
    #[serde(default)]
    pub exports: Vec<String>,
    pub entry_point: Option<String>,
    pub css_bundle: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metafile {
    #[serde(deserialize_with = "unique")]
    pub inputs: BTreeMap<String, Input>,
    #[serde(deserialize_with = "unique")]
    pub outputs: BTreeMap<String, Output>,
}
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub inputs: usize,
    pub outputs: usize,
    pub bytes: u64,
    pub attributed: u64,
    pub unattributed: u64,
    pub over_attributed: u64,
    pub source_bytes: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub kind: &'static str,
    pub path: String,
    pub count: u64,
}
pub struct Snapshot {
    pub source: String,
    pub name: String,
    pub meta: Metafile,
    pub totals: Totals,
    pub diagnostics: Vec<Diagnostic>,
}
pub struct Reports {
    pub before: Snapshot,
    pub after: Option<Snapshot>,
}
impl Reports {
    pub fn side(&self, side: &str) -> PluginResult<&Snapshot> {
        match side {
            "before" => Ok(&self.before),
            "after" => self.after.as_ref().ok_or_else(|| error("after")),
            _ => Err(error("side")),
        }
    }
}
pub fn path_ok(path: &str) -> PluginResult<()> {
    if path.is_empty() || path.len() > 2048 {
        Err(error("path"))
    } else {
        Ok(())
    }
}
fn depth(text: &str, ctx: &dyn TaskContext) -> PluginResult<()> {
    let mut quoted = false;
    let mut escape = false;
    let mut depth = 0usize;
    for (i, b) in text.bytes().enumerate() {
        if i.is_multiple_of(65536) {
            check(ctx)?;
        }
        if quoted {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                quoted = false;
            }
        } else {
            match b {
                b'"' => quoted = true,
                b'[' | b'{' => {
                    depth += 1;
                    if depth > 64 {
                        return Err(error("depth"));
                    }
                }
                b']' | b'}' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    Ok(())
}
pub fn parse(name: String, text: &str, ctx: &dyn TaskContext) -> PluginResult<Snapshot> {
    if text.len() > FILE_LIMIT {
        return Err(error("size"));
    }
    depth(text, ctx)?;
    let meta: Metafile =
        serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| {
            error("json")
                .with("line", e.line())
                .with("column", e.column())
        })?;
    if meta.inputs.len() > 20_000 || meta.outputs.len() > 2_000 {
        return Err(error("entries"));
    }
    let mut totals = Totals {
        inputs: meta.inputs.len(),
        outputs: meta.outputs.len(),
        ..Totals::default()
    };
    let mut diagnostics = Vec::new();
    let mut edges = 0usize;
    let mut contributions = 0usize;
    let mut imports = |items: &[Import], known: &dyn Fn(&str) -> bool| -> PluginResult<u64> {
        edges += items.len();
        if edges > 100_000 {
            return Err(error("entries"));
        }
        let mut missing = 0;
        for item in items {
            check(ctx)?;
            path_ok(&item.path)?;
            if item.kind.is_empty() || item.kind.len() > 128 {
                return Err(error("path"));
            }
            if !item.external && !known(&item.path) {
                missing += 1;
            }
        }
        Ok(missing)
    };
    for (path, input) in &meta.inputs {
        check(ctx)?;
        path_ok(path)?;
        totals.source_bytes = add(totals.source_bytes, input.bytes)?;
        let count = imports(&input.imports, &|s| meta.inputs.contains_key(s))?;
        if count > 0 {
            diagnostics.push(Diagnostic {
                kind: "unknownImport",
                path: path.clone(),
                count,
            });
        }
    }
    for (path, out) in &meta.outputs {
        check(ctx)?;
        path_ok(path)?;
        totals.bytes = add(totals.bytes, out.bytes)?;
        contributions += out.inputs.len();
        if contributions > 100_000 {
            return Err(error("entries"));
        }
        let count = imports(&out.imports, &|s| meta.outputs.contains_key(s))?;
        if count > 0 {
            diagnostics.push(Diagnostic {
                kind: "unknownOutput",
                path: path.clone(),
                count,
            });
        }
        let mut missing = 0;
        let mut sum = 0;
        for (name, c) in &out.inputs {
            check(ctx)?;
            path_ok(name)?;
            sum = add(sum, c.bytes_in_output)?;
            if !meta.inputs.contains_key(name) {
                missing += 1;
            }
        }
        if missing > 0 {
            diagnostics.push(Diagnostic {
                kind: "unknownInput",
                path: path.clone(),
                count: missing,
            });
        }
        for (field, kind) in [
            (&out.entry_point, "unknownEntry"),
            (&out.css_bundle, "unknownCss"),
        ] {
            if let Some(s) = field {
                path_ok(s)?;
                let known = if kind == "unknownEntry" {
                    meta.inputs.contains_key(s)
                } else {
                    meta.outputs.contains_key(s)
                };
                if !known {
                    diagnostics.push(Diagnostic {
                        kind,
                        path: path.clone(),
                        count: 1,
                    });
                }
            }
        }
        if out.exports.len() > 20_000 || out.exports.iter().any(|s| s.len() > 2048) {
            return Err(error("entries"));
        }
        totals.attributed = add(totals.attributed, sum)?;
        if sum <= out.bytes {
            totals.unattributed = add(totals.unattributed, out.bytes - sum)?;
        } else {
            totals.over_attributed = add(totals.over_attributed, sum - out.bytes)?;
            diagnostics.push(Diagnostic {
                kind: "overAttributed",
                path: path.clone(),
                count: sum - out.bytes,
            });
        }
    }
    check(ctx)?;
    Ok(Snapshot {
        source: name.clone(),
        name,
        meta,
        totals,
        diagnostics,
    })
}
