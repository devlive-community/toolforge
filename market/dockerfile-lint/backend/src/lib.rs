//! Dockerfile 静态检查后端：规则集对齐 hadolint 常用项，输出行号、严重级别与
//! 修复示例代码。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct DockerfileLint {
    manifest: Manifest,
}

impl Default for DockerfileLint {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for DockerfileLint {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "lint" => to_value(lint(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(DockerfileLint);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub content: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub line: usize,
    /// 规则键 rules.<key>
    pub rule: &'static str,
    /// severity：error / warning / info
    pub severity: &'static str,
    /// 修复示例（可直接粘贴）
    pub fix: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub issues: Vec<Issue>,
    pub instruction_count: usize,
    pub stages: usize,
}

pub fn lint(args: Args) -> PluginResult<Output> {
    if args.content.trim().is_empty() {
        return Err(PluginError::new("dockerfile.empty"));
    }
    if args.content.len() > 512 * 1024 {
        return Err(PluginError::new("dockerfile.too_large").with("max", 512 * 1024));
    }
    let mut issues: Vec<Issue> = Vec::new();
    let mut stages = 0usize;
    let mut instruction_count = 0usize;
    let mut in_run_continuation = false;
    for (index, raw_line) in args.content.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if in_run_continuation {
            if line.ends_with('\\') {
                continue;
            }
            in_run_continuation = false;
            continue;
        }
        let Some((instruction, rest)) = line.split_once(' ') else {
            instruction_count += 1;
            continue;
        };
        let instruction_upper = instruction.to_uppercase();
        instruction_count += 1;
        match instruction_upper.as_str() {
            "FROM" => {
                let base = rest.trim();
                if base.ends_with(":latest") || !base.contains(':') && !base.contains('@') {
                    issues.push(Issue {
                        line: line_number,
                        rule: "pin_tag",
                        severity: "warning",
                        fix: format!(
                            "FROM {} <!-- pin an explicit version -->",
                            base.split_whitespace().next().unwrap_or("image")
                        ),
                    });
                }
                stages += 1;
            }
            "ADD" => {
                let source = rest.split_whitespace().next().unwrap_or("");
                // ADD 仅在 URL 或自动解压 tar 时合理；本地常规文件用 COPY
                if !source.starts_with("http://")
                    && !source.starts_with("https://")
                    && !source.ends_with(".tar.gz")
                    && !source.ends_with(".tar.xz")
                {
                    issues.push(Issue {
                        line: line_number,
                        rule: "use_copy",
                        severity: "warning",
                        fix: format!("COPY {}", rest.trim()),
                    });
                }
            }
            "RUN" => {
                if rest.contains("apt-get install") && !rest.contains("--no-install-recommends") {
                    issues.push(Issue {
                        line: line_number,
                        rule: "apt_no_install_recommends",
                        severity: "info",
                        fix: format!("RUN {} --no-install-recommends", rest),
                    });
                }
                if rest.contains("apt-get install")
                    && !rest.contains("rm -rf /var/lib/apt/lists")
                    && !line.ends_with('\\')
                {
                    issues.push(Issue {
                        line: line_number,
                        rule: "apt_cache_cleanup",
                        severity: "warning",
                        fix: format!("RUN {} && rm -rf /var/lib/apt/lists/*", rest),
                    });
                }
                if line.ends_with('\\') {
                    in_run_continuation = true;
                }
            }
            "USER" if rest.trim() == "root" => {
                issues.push(Issue {
                    line: line_number,
                    rule: "no_root",
                    severity: "error",
                    fix: "USER appuser".to_owned(),
                });
            }
            _ => {}
        }
    }
    // 全文级规则：HEALTHCHECK 缺失
    if !args.content.to_uppercase().contains("HEALTHCHECK") {
        issues.push(Issue {
            line: 0,
            rule: "no_healthcheck",
            severity: "info",
            fix: "HEALTHCHECK --interval=30s CMD curl -f http://localhost/ || exit 1".to_owned(),
        });
    }
    let stage_note = stages;
    let _ = stage_note;
    issues.sort_by_key(|issue| issue.line);
    Ok(Output {
        issues,
        instruction_count,
        stages,
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
