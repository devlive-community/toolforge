//! GitHub Actions workflow 后端：解析 jobs 与 needs 依赖，构建 DAG，
//! 循环依赖检测与 Mermaid 导出。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct WorkflowDag {
    manifest: Manifest,
}

impl Default for WorkflowDag {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for WorkflowDag {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "parse" => to_value(parse_workflow(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(WorkflowDag);

const MAX_JOBS: usize = 200;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    /// workflow YAML 文本
    pub yaml: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub needs: Vec<String>,
    pub steps: usize,
    /// 触发相关：job 内 if 条件原文（可空）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// 拓扑层级（无依赖为 0）
    pub level: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub name: Option<String>,
    /// 触发事件（push/pull_request/schedule 等）
    pub triggers: Vec<String>,
    pub jobs: Vec<Job>,
    /// 循环依赖的 job id 列表
    pub cycles: Vec<String>,
    pub edges: usize,
    pub mermaid: String,
}

pub fn parse_workflow(args: Args) -> PluginResult<Output> {
    if args.yaml.len() > 512 * 1024 {
        return Err(PluginError::new("workflow.too_large").with("max", 512 * 1024));
    }
    let root: serde_yaml::Value = serde_yaml::from_str(&args.yaml)
        .map_err(|e| PluginError::new("workflow.bad_yaml").with("detail", e.to_string()))?;
    let jobs_value = root.get("jobs").and_then(|j| j.as_mapping());
    let Some(jobs_value) = jobs_value.filter(|j| !j.is_empty()) else {
        return Err(PluginError::new("workflow.no_jobs"));
    };
    let job_count = jobs_value.len();
    if job_count > MAX_JOBS {
        return Err(PluginError::new("workflow.too_many_jobs").with("max", MAX_JOBS as u64));
    }
    // 触发事件
    let mut triggers = Vec::new();
    // YAML 1.1 把裸 `on` 解析成 bool true；两种键都要查
    let on_key = root.as_mapping().and_then(|map| {
        map.keys()
            .find(|key| {
                matches!(key, serde_yaml::Value::String(text) if text == "on")
                    || matches!(key, serde_yaml::Value::Bool(true))
            })
            .cloned()
    });
    if let Some(on) = on_key.and_then(|key| root.get(key)) {
        match on {
            serde_yaml::Value::Mapping(map) => {
                for (key, _) in map {
                    if let Some(name) = key.as_str() {
                        triggers.push(name.to_owned());
                    }
                }
            }
            serde_yaml::Value::Sequence(list) => {
                for item in list {
                    if let Some(name) = item.as_str() {
                        triggers.push(name.to_owned());
                    }
                }
            }
            serde_yaml::Value::String(name) => triggers.push(name.clone()),
            _ => {}
        }
    }
    // jobs
    let mut jobs: Vec<Job> = Vec::new();
    let mut edges = 0usize;
    for (key, value) in jobs_value {
        let Some(id) = key.as_str() else { continue };
        let mut needs: Vec<String> = Vec::new();
        let mut steps = 0usize;
        let mut condition = None;
        if let Some(mapping) = value.as_mapping() {
            if let Some(needs_value) = mapping.get(serde_yaml::Value::from("needs")) {
                match needs_value {
                    serde_yaml::Value::Sequence(list) => {
                        for item in list {
                            if let Some(dep) = item.as_str() {
                                needs.push(dep.to_owned());
                            }
                        }
                    }
                    serde_yaml::Value::String(dep) => needs.push(dep.clone()),
                    _ => {}
                }
            }
            if let Some(steps_value) = mapping
                .get(serde_yaml::Value::from("steps"))
                .and_then(|s| s.as_sequence())
            {
                steps = steps_value.len();
            }
            if let Some(if_value) = mapping
                .get(serde_yaml::Value::from("if"))
                .and_then(|i| i.as_str())
            {
                condition = Some(if_value.to_owned());
            }
        }
        edges += needs.len();
        jobs.push(Job {
            id: id.to_owned(),
            needs,
            steps,
            condition,
            level: 0,
        });
    }
    // 循环依赖检测（DFS）
    let mut cycles: Vec<String> = Vec::new();
    {
        let find_job = |id: &str| jobs.iter().position(|job| job.id == id);
        let mut color: Vec<u8> = vec![0; jobs.len()]; // 0 白 1 灰 2 黑
        for start in 0..jobs.len() {
            if color[start] != 0 {
                continue;
            }
            let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
            color[start] = 1;
            while let Some((node, dep_index)) = stack.pop() {
                let needs = jobs[node].needs.clone();
                if dep_index < needs.len() {
                    stack.push((node, dep_index + 1));
                    if let Some(next) = find_job(&needs[dep_index]) {
                        match color[next] {
                            1 => {
                                let cycle = jobs[next].id.clone();
                                if !cycles.contains(&cycle) {
                                    cycles.push(cycle);
                                }
                            }
                            0 => {
                                color[next] = 1;
                                stack.push((next, 0));
                            }
                            _ => {}
                        }
                    }
                } else {
                    color[node] = 2;
                }
            }
        }
    }
    // 拓扑层级（无环时）
    if cycles.is_empty() {
        loop {
            let mut changed = false;
            let levels: Vec<(String, usize)> =
                jobs.iter().map(|job| (job.id.clone(), job.level)).collect();
            for job in &mut jobs {
                let new_level = levels
                    .iter()
                    .filter(|(id, _)| job.needs.contains(id))
                    .map(|(_, level)| level + 1)
                    .max()
                    .unwrap_or(0);
                if new_level != job.level {
                    job.level = new_level;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }
    // Mermaid
    let mut mermaid = String::from("flowchart TD\n");
    for job in &jobs {
        mermaid.push_str(&format!(
            "  {}[\"{}\"]\n",
            sanitize_mermaid_id(&job.id),
            job.id
        ));
    }
    for job in &jobs {
        for need in &job.needs {
            mermaid.push_str(&format!(
                "  {} --> {}\n",
                sanitize_mermaid_id(need),
                sanitize_mermaid_id(&job.id)
            ));
        }
    }
    Ok(Output {
        name: root.get("name").and_then(|n| n.as_str()).map(str::to_owned),
        triggers,
        jobs,
        cycles,
        edges,
        mermaid,
    })
}

fn sanitize_mermaid_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
