//! 根据 Conventional Commits 生成发布说明。
//!
//! 发布说明同时写入 GitHub Release 与 latest.json，应用内更新对话框会展示它。

use std::path::Path;
use std::process::Command;

struct Section {
    title: &'static str,
    types: &'static [&'static str],
}

const SECTIONS: &[Section] = &[
    Section {
        title: "Features",
        types: &["feat"],
    },
    Section {
        title: "Bug Fixes",
        types: &["fix"],
    },
    Section {
        title: "Performance",
        types: &["perf"],
    },
    Section {
        title: "Internationalization",
        types: &["i18n"],
    },
    Section {
        title: "Refactoring",
        types: &["refactor"],
    },
];

/// 解析提交标题：返回 (type, scope, subject)；不符合规范的返回 None
pub fn parse_subject(subject: &str) -> Option<(&str, Option<&str>, &str)> {
    let (head, rest) = subject.split_once(": ")?;
    let head = head.trim_end_matches('!');
    let (kind, scope) = match head.split_once('(') {
        Some((kind, scope)) => (kind, Some(scope.strip_suffix(')')?)),
        None => (head, None),
    };
    (!kind.is_empty() && kind.chars().all(|c| c.is_ascii_lowercase()))
        .then_some((kind, scope, rest))
}

/// 把提交标题列表渲染为 Markdown；chore / ci / test / docs / style 不进入发布说明
pub fn render(subjects: &[String]) -> String {
    let mut out = String::new();
    for section in SECTIONS {
        let items: Vec<String> = subjects
            .iter()
            .filter_map(|s| parse_subject(s))
            .filter(|(kind, _, _)| section.types.contains(kind))
            .map(|(_, scope, subject)| match scope {
                Some(scope) => format!("- **{scope}**: {subject}"),
                None => format!("- {subject}"),
            })
            .collect();
        if items.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("### {}\n\n{}\n", section.title, items.join("\n")));
    }
    if out.is_empty() {
        out.push_str("Maintenance release.\n");
    }
    out
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// 生成 `to`（默认 HEAD）相对上一个 v* 标签的发布说明
pub fn run(root: &Path, to: Option<&str>) -> Result<String, String> {
    let to = to.unwrap_or("HEAD");
    let previous = git(
        root,
        &[
            "describe",
            "--tags",
            "--abbrev=0",
            "--match",
            "v*",
            &format!("{to}^"),
        ],
    )
    .ok();
    let range = match &previous {
        Some(tag) => format!("{tag}..{to}"),
        None => to.to_owned(),
    };
    let log = git(root, &["log", "--no-merges", "--pretty=%s", &range])?;
    let subjects: Vec<String> = log.lines().map(str::to_owned).collect();
    Ok(render(&subjects))
}

#[cfg(test)]
#[path = "notes_test.rs"]
mod tests;
