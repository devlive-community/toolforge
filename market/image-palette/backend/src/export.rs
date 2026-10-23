//! 把调色板导出为常用格式。

use serde::Deserialize;

use crate::color;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    Css,
    Scss,
    Tailwind,
    Json,
    Gpl,
    Hex,
}

pub fn export(colors: &[[u8; 3]], format: Format, name: &str) -> String {
    let hexes: Vec<String> = colors.iter().map(|c| color::hex(*c)).collect();
    let numbered = |i: usize| format!("{}", (i + 1) * 100);
    match format {
        Format::Css => {
            let lines: Vec<String> = hexes
                .iter()
                .enumerate()
                .map(|(i, h)| format!("  --{name}-{}: {h};", numbered(i)))
                .collect();
            format!(":root {{\n{}\n}}", lines.join("\n"))
        }
        Format::Scss => hexes
            .iter()
            .enumerate()
            .map(|(i, h)| format!("${name}-{}: {h};", numbered(i)))
            .collect::<Vec<_>>()
            .join("\n"),
        Format::Tailwind => {
            let lines: Vec<String> = hexes
                .iter()
                .enumerate()
                .map(|(i, h)| format!("      {}: '{h}',", numbered(i)))
                .collect();
            format!(
                "// tailwind.config.js\ntheme: {{\n  extend: {{\n    colors: {{\n      {name}: {{\n{}\n      }},\n    }},\n  }},\n}}",
                lines
                    .iter()
                    .map(|l| format!("  {l}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        }
        Format::Json => serde_json::to_string_pretty(&hexes).unwrap_or_default(),
        Format::Gpl => {
            let lines: Vec<String> = colors
                .iter()
                .zip(&hexes)
                .map(|([r, g, b], h)| format!("{r:>3} {g:>3} {b:>3}\t{h}"))
                .collect();
            format!(
                "GIMP Palette\nName: {name}\nColumns: {}\n#\n{}",
                colors.len().min(16),
                lines.join("\n")
            )
        }
        Format::Hex => hexes.join("\n"),
    }
}

#[cfg(test)]
#[path = "export_test.rs"]
mod tests;
