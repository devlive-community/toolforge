//! 约定扫描：把 CLAUDE/项目约定中的强制规则变成可执行的检查。
//!
//! 某一行确属例外时，在该行加上 `tf-allow: <原因>` 注释即可跳过。

use std::path::Path;

use regex::Regex;
use walkdir::{DirEntry, WalkDir};

const SCAN_ROOTS: &[&str] = &["apps", "packages", "plugins", "crates"];
const SKIP_DIRS: &[&str] = &["node_modules", "target", "dist", "gen", ".git"];
const ALLOW_MARKER: &str = "tf-allow";

/// 前端禁止依赖的数据处理类库（数据处理必须在 Rust 侧完成）
pub const BANNED_WEB_DEPS: &[&str] = &[
    "ajv",
    "crypto-js",
    "date-fns",
    "dayjs",
    "diff",
    "fast-xml-parser",
    "js-base64",
    "js-yaml",
    "json5",
    "jsonc-parser",
    "lodash",
    "moment",
    "nanoid",
    "papaparse",
    "sql-formatter",
    "uuid",
    "xml2js",
    "yaml",
];

pub struct Rule {
    pub id: &'static str,
    pub message: &'static str,
    extensions: &'static [&'static str],
    /// 路径包含这些片段的文件不参与该规则
    exclude: &'static [&'static str],
    pattern: Regex,
}

impl Rule {
    fn applies_to(&self, path: &str) -> bool {
        let ext_ok = self.extensions.iter().any(|ext| path.ends_with(ext));
        ext_ok && !self.exclude.iter().any(|part| path.contains(part))
    }

    pub fn matches(&self, line: &str) -> bool {
        !line.contains(ALLOW_MARKER) && self.pattern.is_match(line)
    }
}

pub fn rules() -> Vec<Rule> {
    let rule = |id, message, extensions, exclude, pattern: &str| Rule {
        id,
        message,
        extensions,
        exclude,
        pattern: Regex::new(pattern).expect("valid rule pattern"),
    };
    vec![
        rule(
            "rust-inline-tests",
            "Rust tests must live in a dedicated *_test.rs file",
            &[".rs"],
            &["_test.rs"],
            r"^\s*mod\s+tests\s*\{",
        ),
        rule(
            "browser-storage",
            "Browser storage is forbidden; persist through SQLite in Rust",
            &[".ts", ".tsx", ".js", ".jsx", ".html"],
            &[],
            r"\b(localStorage|sessionStorage|indexedDB)\b|document\.cookie",
        ),
        rule(
            "frontend-data-processing",
            "Data processing must run in Rust, not in the frontend",
            &[".ts", ".tsx"],
            &[],
            r"\b(atob|btoa)\(|crypto\.subtle|JSON\.parse\(",
        ),
        rule(
            "native-controls",
            "Use @toolforge/ui components instead of native form controls and dialogs",
            &[".tsx"],
            &["packages/ui/src/"],
            r#"<(select|datalist|dialog|textarea|input)\b|type=["'](checkbox|radio|range|number|date|time|datetime-local|color|file)["']|window\.(alert|confirm|prompt)\("#,
        ),
        rule(
            "raw-palette",
            "Use semantic design tokens instead of raw palette classes or hard-coded colors",
            &[".ts", ".tsx"],
            &[],
            r#"\b(bg|text|border|ring|fill|stroke|from|to|via|outline|divide|shadow|decoration|accent|caret)-(slate|gray|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose|black|white)(-\d{2,3})?\b|-\[(#|rgb|hsl|oklch)|["'`]#[0-9a-fA-F]{3,8}["'`]"#,
        ),
        rule(
            "dark-variant",
            "Do not use dark: variants; dark mode is handled by tokens",
            &[".ts", ".tsx"],
            &[],
            r"\bdark:",
        ),
    ]
}

pub struct Violation {
    pub rule: &'static str,
    pub message: String,
    pub location: String,
}

fn is_skipped(entry: &DirEntry) -> bool {
    entry.file_type().is_dir() && SKIP_DIRS.contains(&entry.file_name().to_string_lossy().as_ref())
}

pub fn scan(root: &Path) -> Vec<Violation> {
    let rules = rules();
    let mut violations = Vec::new();

    for base in SCAN_ROOTS {
        let walker = WalkDir::new(root.join(base)).into_iter();
        for entry in walker.filter_entry(|e| !is_skipped(e)).flatten() {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");

            if relative.ends_with("package.json") {
                violations.extend(check_package_json(path, &relative));
                continue;
            }

            let applicable: Vec<&Rule> = rules.iter().filter(|r| r.applies_to(&relative)).collect();
            if applicable.is_empty() {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(path) else {
                continue;
            };
            for (index, line) in content.lines().enumerate() {
                for rule in applicable.iter().filter(|r| r.matches(line)) {
                    violations.push(Violation {
                        rule: rule.id,
                        message: rule.message.to_owned(),
                        location: format!("{relative}:{}", index + 1),
                    });
                }
            }
        }
    }
    violations
}

/// 依赖名以 `"name":` 形式出现在 dependencies / devDependencies 中即视为违规
pub fn banned_deps_in(content: &str) -> Vec<&'static str> {
    BANNED_WEB_DEPS
        .iter()
        .copied()
        .filter(|dep| content.contains(&format!("\"{dep}\":")))
        .collect()
}

fn check_package_json(path: &Path, relative: &str) -> Vec<Violation> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    banned_deps_in(&content)
        .into_iter()
        .map(|dep| Violation {
            rule: "banned-web-dependency",
            message: format!("`{dep}` processes data in the frontend; implement it in Rust"),
            location: relative.to_owned(),
        })
        .collect()
}

pub fn run(root: &Path) -> Result<(), String> {
    let violations = scan(root);
    if violations.is_empty() {
        println!("\x1b[32m✓\x1b[0m convention rules");
        return Ok(());
    }
    for v in &violations {
        println!("  \x1b[33m{}\x1b[0m [{}] {}", v.location, v.rule, v.message);
        // GitHub Actions 注解，直接标在 PR 的对应行上
        if std::env::var_os("GITHUB_ACTIONS").is_some() {
            let (file, line) = v.location.rsplit_once(':').unwrap_or((&v.location, "1"));
            println!(
                "::error file={file},line={line},title={}::{}",
                v.rule, v.message
            );
        }
    }
    Err(format!(
        "{} convention violation(s) found",
        violations.len()
    ))
}

#[cfg(test)]
#[path = "rules_test.rs"]
mod tests;
