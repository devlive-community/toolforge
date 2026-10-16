//! Pure transformations for the library persisted by the host's SQLite store.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::generate::{is_rule, source_lines};

pub const MAX_TEMPLATES: usize = 64;
pub const MAX_CONTENT_BYTES: usize = 65_536;
pub const MAX_LIBRARY_BYTES: usize = 262_144;
const MAX_NAME_CHARACTERS: usize = 80;
// Reserve the final safe integer as an exhausted nextId sentinel. IPC numbers
// round-trip through JavaScript, so larger u64 IDs would silently lose identity.
const MAX_NEXT_ID: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub content: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Library {
    pub version: u8,
    pub next_id: u64,
    pub templates: Vec<Template>,
}

impl Default for Library {
    fn default() -> Self {
        Self {
            version: 1,
            next_id: 1,
            templates: Vec::new(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Validate,
    Save,
    Delete,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Args {
    pub operation: Operation,
    #[serde(default)]
    pub library: Library,
    pub id: Option<String>,
    pub name: Option<String>,
    pub content: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub library: Library,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,
}

fn invalid(field: &str) -> PluginError {
    PluginError::new("gitignore.libraryInvalid").with("field", field)
}

fn template_number(id: &str) -> PluginResult<u64> {
    let number = id
        .strip_prefix("custom-")
        .filter(|number| !number.starts_with('0'))
        .and_then(|number| number.parse::<u64>().ok())
        .filter(|number| *number > 0 && *number < MAX_NEXT_ID)
        .ok_or_else(|| invalid("id"))?;
    // Parsing accepts +1; IDs must be canonical, stable keys.
    if id != format!("custom-{number}") {
        return Err(invalid("id"));
    }
    Ok(number)
}

fn normalized_name(name: &str) -> String {
    name.trim().to_lowercase()
}

fn validate_template(template: &Template) -> PluginResult<()> {
    if template.name.trim().is_empty()
        || template.name.chars().count() > MAX_NAME_CHARACTERS
        || template
            .name
            .chars()
            .any(|character| character.is_control() || matches!(character, '\u{2028}' | '\u{2029}'))
    {
        return Err(
            PluginError::new("gitignore.libraryInvalidName").with("max", MAX_NAME_CHARACTERS)
        );
    }
    if template.content.len() > MAX_CONTENT_BYTES {
        return Err(PluginError::new("gitignore.libraryContentTooLarge")
            .with("max", MAX_CONTENT_BYTES)
            .with("name", template.name.clone()));
    }
    if template.content.contains('\0') {
        return Err(
            PluginError::new("gitignore.libraryInvalidContent").with("name", template.name.clone())
        );
    }
    if !source_lines(&template.content).any(is_rule) {
        return Err(
            PluginError::new("gitignore.libraryEmptyContent").with("name", template.name.clone())
        );
    }
    Ok(())
}

/// Validation never repairs, drops or rewrites persisted data. Invalid storage
/// must remain visible to the caller so a load cannot silently overwrite it.
pub fn validate(library: &Library) -> PluginResult<()> {
    if library.version != 1 {
        return Err(PluginError::new("gitignore.libraryUnsupportedVersion")
            .with("version", library.version));
    }
    if library.next_id == 0 || library.next_id > MAX_NEXT_ID {
        return Err(invalid("nextId"));
    }
    if library.templates.len() > MAX_TEMPLATES {
        return Err(PluginError::new("gitignore.libraryTooMany").with("max", MAX_TEMPLATES));
    }
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    let mut total = 0usize;
    for template in &library.templates {
        if template_number(&template.id)? >= library.next_id {
            return Err(invalid("nextId"));
        }
        if !ids.insert(&template.id) {
            return Err(invalid("id"));
        }
        validate_template(template)?;
        if !names.insert(normalized_name(&template.name)) {
            return Err(PluginError::new("gitignore.libraryDuplicateName")
                .with("name", template.name.clone()));
        }
        total += template.content.len();
    }
    if total > MAX_LIBRARY_BYTES {
        return Err(PluginError::new("gitignore.libraryTooLarge").with("max", MAX_LIBRARY_BYTES));
    }
    // At most 256 KiB of raw content can expand sixfold under JSON escaping.
    // IDs and bounded control-free names keep the complete serialized library
    // below the host SQLite plugin-state limit of 2 MiB.
    Ok(())
}

pub fn run(args: Args) -> PluginResult<Output> {
    validate(&args.library)?;
    let mut library = args.library;
    let mut template_id = None;
    match args.operation {
        Operation::Validate => {}
        Operation::Save => {
            let name = args.name.ok_or_else(|| invalid("name"))?;
            let content = args.content.ok_or_else(|| invalid("content"))?;
            if let Some(id) = args.id {
                let template = library
                    .templates
                    .iter_mut()
                    .find(|template| template.id == id)
                    .ok_or_else(|| {
                        PluginError::new("gitignore.libraryUnknownTemplate").with("id", id.clone())
                    })?;
                template.name = name;
                template.content = content;
                template_id = Some(id);
            } else {
                if library.next_id == MAX_NEXT_ID {
                    return Err(PluginError::new("gitignore.libraryIdExhausted"));
                }
                let id = format!("custom-{}", library.next_id);
                library.next_id += 1;
                library.templates.push(Template {
                    id: id.clone(),
                    name,
                    content,
                });
                template_id = Some(id);
            }
        }
        Operation::Delete => {
            let id = args.id.ok_or_else(|| invalid("id"))?;
            let index = library
                .templates
                .iter()
                .position(|template| template.id == id)
                .ok_or_else(|| {
                    PluginError::new("gitignore.libraryUnknownTemplate").with("id", id)
                })?;
            library.templates.remove(index);
        }
    }
    validate(&library)?;
    Ok(Output {
        library,
        template_id,
    })
}

#[cfg(test)]
#[path = "library_test.rs"]
mod tests;
