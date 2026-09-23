//! 生成发布说明：版本的完整提交历史（上一个 v* 标签到本次），按 Conventional Commits 类型分组。
//!
//! 发布说明写入附注标签，由 release 工作流作为 GitHub Release 正文与 latest.json 的 notes，
//! 应用内更新对话框会展示它。

use std::path::Path;
use std::process::Command;

const REPOSITORY: &str = "https://github.com/devlive-community/toolforge";

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
    Section {
        title: "Documentation",
        types: &["docs"],
    },
    Section {
        title: "Tests",
        types: &["test"],
    },
    Section {
        title: "Build & CI",
        types: &["ci", "build"],
    },
    Section {
        title: "Chores",
        types: &["chore", "style"],
    },
];

/// 一条提交：短哈希 + 标题
#[derive(Debug, Clone, PartialEq)]
pub struct Commit {
    pub hash: String,
    pub subject: String,
}

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

/// 版本发布提交本身不进入说明
fn is_release_commit(subject: &str) -> bool {
    matches!(parse_subject(subject), Some(("chore", Some("release"), _)))
}

fn line(commit: &Commit, scope: Option<&str>, subject: &str) -> String {
    match scope {
        Some(scope) => format!("- **{scope}**: {subject} ({})", commit.hash),
        None => format!("- {subject} ({})", commit.hash),
    }
}

/// 渲染 Markdown；`compare` 为 (上一个标签, 本次标签)，用于生成完整变更对比链接
pub fn render(commits: &[Commit], compare: Option<(&str, &str)>) -> String {
    let commits: Vec<&Commit> = commits
        .iter()
        .filter(|c| !is_release_commit(&c.subject))
        .collect();
    let mut blocks = Vec::new();

    for section in SECTIONS {
        let items: Vec<String> = commits
            .iter()
            .filter_map(|c| parse_subject(&c.subject).map(|p| (c, p)))
            .filter(|(_, (kind, _, _))| section.types.contains(kind))
            .map(|(c, (_, scope, subject))| line(c, scope, subject))
            .collect();
        if !items.is_empty() {
            blocks.push(format!("### {}\n\n{}", section.title, items.join("\n")));
        }
    }

    let others: Vec<String> = commits
        .iter()
        .filter(|c| match parse_subject(&c.subject) {
            Some((kind, _, _)) => !SECTIONS.iter().any(|s| s.types.contains(&kind)),
            None => true,
        })
        .map(|c| line(c, None, &c.subject))
        .collect();
    if !others.is_empty() {
        blocks.push(format!("### Other Changes\n\n{}", others.join("\n")));
    }

    if blocks.is_empty() {
        blocks.push("Maintenance release.".to_owned());
    }
    if let Some((previous, current)) = compare {
        blocks.push(format!(
            "**Full Changelog**: {REPOSITORY}/compare/{previous}...{current}"
        ));
    }
    format!("{}\n", blocks.join("\n\n"))
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

/// 生成 `to`（默认 HEAD）相对上一个 v* 标签的发布说明；`tag` 为本次版本的标签名（用于对比链接）
pub fn run(root: &Path, to: Option<&str>, tag: Option<&str>) -> Result<String, String> {
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
        Some(prev) => format!("{prev}..{to}"),
        None => to.to_owned(),
    };
    let log = git(root, &["log", "--no-merges", "--pretty=%h%x09%s", &range])?;
    let commits: Vec<Commit> = log
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(hash, subject)| Commit {
            hash: hash.to_owned(),
            subject: subject.to_owned(),
        })
        .collect();
    let current = tag.or(to.starts_with('v').then_some(to));
    let compare = previous.as_deref().zip(current);
    Ok(render(&commits, compare))
}

#[cfg(test)]
#[path = "notes_test.rs"]
mod tests;
