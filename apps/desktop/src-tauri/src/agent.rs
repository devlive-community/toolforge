//! 插件的命令行与 AI Agent 配置。全局项（AI Agent 总开关、MCP 接入、授权目录）在设置页，
//! 按插件、按函数的开关在工具市场的工具配置弹窗中：插件与函数默认开启，用户可以关闭。
//! 授权判断规则见 tf_host::agent 与 tf_host::cli_access。

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Value, json};
use tauri::State;
use tf_core::store::PLUGIN_SCOPE;
use tf_core::{AgentPath, AppError, AppResult, InterfaceGrant};
use tf_host::agent::{ENABLED_KEY, Mode, Policy};
use tf_host::cli_access::CliAccess;
use tf_host::cli_links;
use tf_plugin_api::Manifest;
use tf_plugin_api::interface::{Channel, Effect, Interface};

use crate::AppState;

const FS_PERMISSION: &str = "fs:user-selected";

fn effect_name(effect: Effect) -> &'static str {
    match effect {
        Effect::Read => "read",
        Effect::Write => "write",
        Effect::External => "external",
    }
}

fn channel(name: &str) -> AppResult<Channel> {
    match name {
        "cli" => Ok(Channel::Cli),
        "agent" => Ok(Channel::Agent),
        _ => Err(AppError::new("agent.invalid_channel").with("channel", name)),
    }
}

fn channel_name(channel: Channel) -> &'static str {
    match channel {
        Channel::Agent => tf_host::agent::CHANNEL,
        _ => tf_host::cli_access::CHANNEL,
    }
}

/// 一个函数的接口声明，供插件详情展示
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceInfo {
    pub function: String,
    /// `i18n:` 键，由前端在插件命名空间内翻译
    pub summary: String,
    pub channels: Vec<&'static str>,
    pub effect: &'static str,
    pub uses: Vec<String>,
    pub confirm: bool,
}

/// 插件开放了哪些通道
pub fn channels(manifest: &Manifest) -> Vec<&'static str> {
    [(Channel::Cli, "cli"), (Channel::Agent, "agent")]
        .into_iter()
        .filter(|(c, _)| {
            manifest
                .functions
                .values()
                .any(|spec| spec.declaration().exposed(*c).is_some())
        })
        .map(|(_, name)| name)
        .collect()
}

pub fn interfaces(manifest: &Manifest) -> Vec<InterfaceInfo> {
    manifest
        .functions
        .iter()
        .filter_map(|(name, spec)| {
            let declaration = spec.declaration();
            let interface = declaration
                .exposed(Channel::Cli)
                .or_else(|| declaration.exposed(Channel::Agent))?;
            let channels = [(Channel::Cli, "cli"), (Channel::Agent, "agent")]
                .into_iter()
                .filter(|(c, _)| declaration.exposed(*c).is_some())
                .map(|(_, n)| n)
                .collect();
            Some(InterfaceInfo {
                function: name.clone(),
                summary: interface.summary.clone(),
                channels,
                effect: effect_name(interface.effect),
                uses: interface.uses.clone(),
                confirm: interface.confirm,
            })
        })
        .collect()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionConfig {
    pub name: String,
    pub summary: String,
    pub effect: &'static str,
    pub uses: Vec<String>,
    /// 实际生效的模式；命令行只有 allow / off
    pub mode: Mode,
    /// 没有单独设置时的模式
    pub default_mode: Mode,
    /// 用户的设置；None 为默认
    pub custom: Option<Mode>,
    /// AI Agent：声明要求确认或原生插件用到文件权限，不能设为始终允许
    pub capped: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelConfig {
    pub enabled: bool,
    /// 全局功能已开启：命令行已生成命令 / AI Agent 总开关已打开
    pub ready: bool,
    pub functions: Vec<FunctionConfig>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInterfaceConfig {
    pub id: String,
    pub sensitive: bool,
    /// 原生内置插件，不在 WASM 沙箱中
    pub native: bool,
    /// 插件没有开放该通道时为 None
    pub cli: Option<ChannelConfig>,
    pub agent: Option<ChannelConfig>,
}

fn declared(manifest: &Manifest, channel: Channel) -> Vec<(String, Interface)> {
    manifest
        .functions
        .iter()
        .filter_map(|(name, spec)| {
            let interface = spec.declaration().exposed(channel)?.clone();
            Some((name.clone(), interface))
        })
        .collect()
}

pub fn config(state: &AppState, plugin_id: &str) -> AppResult<PluginInterfaceConfig> {
    let plugin = state.plugins.get(plugin_id)?;
    let manifest = plugin.manifest();
    let native = state.builtin.contains(plugin_id);

    let cli_grants = state.store.interface_grants(tf_host::cli_access::CHANNEL)?;
    let access = CliAccess::load(Some(&state.store))?;
    let cli_functions = declared(manifest, Channel::Cli);
    let cli = (!cli_functions.is_empty()).then(|| ChannelConfig {
        enabled: access.plugin_enabled(plugin_id),
        ready: cli_links::bin_dir().is_some_and(|dir| cli_links::installed(&dir)),
        functions: cli_functions
            .into_iter()
            .map(|(name, interface)| {
                let custom = cli_grants
                    .iter()
                    .find(|g| g.plugin_id == plugin_id && g.function == name)
                    .and_then(|g| Mode::parse(&g.mode));
                FunctionConfig {
                    mode: if access.function_enabled(plugin_id, &name) {
                        Mode::Allow
                    } else {
                        Mode::Off
                    },
                    default_mode: Mode::Allow,
                    custom,
                    capped: false,
                    summary: interface.summary,
                    effect: effect_name(interface.effect),
                    uses: interface.uses,
                    name,
                }
            })
            .collect(),
    });

    let policy = Policy::load(&state.store)?;
    let agent_grants = state.store.interface_grants(tf_host::agent::CHANNEL)?;
    let agent_functions = declared(manifest, Channel::Agent);
    let agent = (!agent_functions.is_empty()).then(|| ChannelConfig {
        enabled: policy.plugin_enabled(plugin_id),
        ready: policy.enabled(),
        functions: agent_functions
            .into_iter()
            .map(|(name, interface)| {
                let custom = agent_grants
                    .iter()
                    .find(|g| g.plugin_id == plugin_id && g.function == name)
                    .and_then(|g| Mode::parse(&g.mode));
                let capped = interface.confirm
                    || (native && interface.uses.iter().any(|u| u == FS_PERMISSION));
                FunctionConfig {
                    mode: policy.function_mode(plugin_id, &name, &interface, native),
                    default_mode: if interface.effect == Effect::Read && !capped {
                        Mode::Allow
                    } else {
                        Mode::Ask
                    },
                    custom,
                    capped,
                    summary: interface.summary,
                    effect: effect_name(interface.effect),
                    uses: interface.uses,
                    name,
                }
            })
            .collect(),
    });

    Ok(PluginInterfaceConfig {
        id: plugin_id.to_owned(),
        sensitive: manifest.sensitive,
        native,
        cli,
        agent,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentOverview {
    pub enabled: bool,
    /// 客户端启动 MCP 服务的命令（安装包里的 tfp）；开发时未编译 tfp 则为空
    pub command: Option<String>,
    /// 写入 MCP 客户端配置的 JSON
    pub config: Option<String>,
    pub paths: Vec<AgentPath>,
}

pub fn overview(state: &AppState) -> AppResult<AgentOverview> {
    let command = std::env::current_exe()
        .ok()
        .map(|exe| cli_links::sidecar_beside(&exe))
        .filter(|p| p.exists())
        .map(|p| p.to_string_lossy().into_owned());
    let config = command.as_ref().map(|command| {
        let value =
            json!({ "mcpServers": { "toolforge": { "command": command, "args": ["mcp"] } } });
        serde_json::to_string_pretty(&value).unwrap_or_default()
    });
    Ok(AgentOverview {
        enabled: Policy::load(&state.store)?.enabled(),
        command,
        config,
        paths: state.store.agent_paths()?,
    })
}

/// 插件 id → 开放的通道；没有声明的旧插件不在其中
#[tauri::command]
pub fn interface_support(state: State<'_, AppState>) -> BTreeMap<String, Vec<&'static str>> {
    state
        .plugins
        .manifests()
        .iter()
        .map(|m| (m.id.clone(), channels(m)))
        .filter(|(_, channels)| !channels.is_empty())
        .collect()
}

#[tauri::command]
pub fn plugin_interfaces(
    state: State<'_, AppState>,
    plugin_id: String,
) -> AppResult<Vec<InterfaceInfo>> {
    Ok(interfaces(state.plugins.get(&plugin_id)?.manifest()))
}

#[tauri::command]
pub fn plugin_interface_config(
    state: State<'_, AppState>,
    plugin_id: String,
) -> AppResult<PluginInterfaceConfig> {
    config(&state, &plugin_id)
}

/// 插件级开关：开启即删除关闭记录（默认开启）
#[tauri::command]
pub fn interface_set_plugin(
    state: State<'_, AppState>,
    channel_id: String,
    plugin_id: String,
    enabled: bool,
) -> AppResult<PluginInterfaceConfig> {
    let channel = channel(&channel_id)?;
    state.plugins.get(&plugin_id)?;
    let name = channel_name(channel);
    if enabled {
        state
            .store
            .interface_grant_remove(name, &plugin_id, PLUGIN_SCOPE)?;
    } else {
        state.store.interface_grant_set(&InterfaceGrant {
            channel: name.to_owned(),
            plugin_id: plugin_id.clone(),
            function: PLUGIN_SCOPE.to_owned(),
            mode: Mode::Off.as_str().to_owned(),
            uses: None,
        })?;
    }
    if channel == Channel::Cli {
        crate::cli_tools::refresh(&state.plugins, &state.store);
    }
    config(&state, &plugin_id)
}

/// 函数级设置；mode 为 None 时恢复默认。命令行只接受 off，AI Agent 授权时记下函数当前声明的权限
#[tauri::command]
pub fn interface_set_function(
    state: State<'_, AppState>,
    channel_id: String,
    plugin_id: String,
    function: String,
    mode: Option<String>,
) -> AppResult<PluginInterfaceConfig> {
    let channel = channel(&channel_id)?;
    let plugin = state.plugins.get(&plugin_id)?;
    let interface = plugin
        .manifest()
        .functions
        .get(&function)
        .and_then(|spec| spec.declaration().exposed(channel).cloned())
        .ok_or_else(|| {
            AppError::new("plugin.function_not_found").with("function", function.as_str())
        })?;
    let name = channel_name(channel);
    match mode {
        None => state
            .store
            .interface_grant_remove(name, &plugin_id, &function)?,
        Some(mode) => {
            let mode = Mode::parse(&mode)
                .filter(|m| channel == Channel::Agent || *m == Mode::Off)
                .ok_or_else(|| AppError::new("agent.invalid_mode"))?;
            state.store.interface_grant_set(&InterfaceGrant {
                channel: name.to_owned(),
                plugin_id: plugin_id.clone(),
                function,
                mode: mode.as_str().to_owned(),
                uses: Some(interface.uses),
            })?;
        }
    }
    config(&state, &plugin_id)
}

#[tauri::command]
pub fn agent_overview(state: State<'_, AppState>) -> AppResult<AgentOverview> {
    overview(&state)
}

#[tauri::command]
pub fn agent_set_enabled(state: State<'_, AppState>, enabled: bool) -> AppResult<AgentOverview> {
    state.store.kv_set(ENABLED_KEY, &Value::Bool(enabled))?;
    overview(&state)
}

#[tauri::command]
pub fn agent_add_path(
    state: State<'_, AppState>,
    path: String,
    write: bool,
) -> AppResult<AgentOverview> {
    let real = std::path::Path::new(&path).canonicalize()?;
    if !real.is_dir() {
        return Err(AppError::new("fs.not_a_directory").with("path", path));
    }
    state.store.agent_path_set(&AgentPath {
        path: real.to_string_lossy().into_owned(),
        access: if write { "write" } else { "read" }.to_owned(),
    })?;
    overview(&state)
}

#[tauri::command]
pub fn agent_remove_path(state: State<'_, AppState>, path: String) -> AppResult<AgentOverview> {
    state.store.agent_path_remove(&path)?;
    overview(&state)
}

#[cfg(test)]
#[path = "agent_test.rs"]
mod tests;
