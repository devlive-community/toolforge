//! HTTP 客户端插件后端（在线安装插件）：请求由宿主发送（任务，可取消、实时日志），
//! 响应正文在插件中格式化并缓存，前端只展示预览。

mod detect;
mod http;
mod import;
mod oauth;
mod workspace;

use serde::Deserialize;
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, parse_args, to_value,
    unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct HttpClient {
    manifest: Manifest,
    bodies: http::Bodies,
    oauth: oauth::OAuth,
}

impl Default for HttpClient {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
            bodies: http::Bodies::default(),
            oauth: oauth::OAuth::default(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RefreshArgs {
    config: oauth::OAuthConfig,
    #[serde(default)]
    variables: Vec<workspace::Variable>,
    #[serde(default)]
    environment_id: Option<String>,
}

impl HttpClient {
    /// 解析请求；绑定了授权时注入 Bearer，已有 Authorization 头视为冲突
    fn prepare(&self, args: &workspace::RequestArgs) -> PluginResult<http::Request> {
        let mut request = args.resolve()?;
        if let Some(session) = &args.oauth {
            if request
                .headers
                .iter()
                .any(|p| p.enabled && p.key.trim().eq_ignore_ascii_case("authorization"))
            {
                return Err(PluginError::new("oauth.header_conflict"));
            }
            let bearer = self.oauth.bearer(&session.key())?;
            request.headers.push(http::Pair {
                key: "Authorization".into(),
                value: bearer.to_string(),
                enabled: true,
            });
        }
        Ok(request)
    }
}

impl ToolPlugin for HttpClient {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn detect(&self, text: &str) -> Option<tf_plugin_api::Detection> {
        detect::detect(text)
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "to_curl" => {
                let args: workspace::RequestArgs = parse_args(args)?;
                to_value(http::to_curl(args.resolve()?).map_err(|error| args.protect_error(error))?)
            }
            "save_body" => to_value(self.bodies.save(parse_args(args)?)?),
            "workspace_validate" => {
                let args: workspace::WorkspaceArgs = parse_args(args)?;
                to_value(workspace::validate(args.workspace)?)
            }
            "new_id" => to_value(workspace::new_id()),
            "resolve_request" => {
                let args: workspace::ResolveArgs = parse_args(args)?;
                to_value(workspace::resolve(args.request, &args.variables)?)
            }
            "import_curl" => {
                let args: import::ImportArgs = parse_args(args)?;
                to_value(import::import(&args.command)?)
            }
            "oauth_begin" => to_value(self.oauth.begin(parse_args(args)?)?),
            "oauth_cancel" => {
                self.oauth.cancel(parse_args::<oauth::FlowArgs>(args)?.flow);
                Ok(Value::Null)
            }
            "oauth_sessions" => {
                let args: oauth::SessionsArgs = parse_args(args)?;
                to_value(self.oauth.views(&args.keys))
            }
            "oauth_clear" => {
                let args: oauth::SessionArgs = parse_args(args)?;
                to_value(self.oauth.clear(&args.key()))
            }
            "oauth_reveal" => {
                let args: oauth::SessionArgs = parse_args(args)?;
                to_value(self.oauth.reveal(&args.key())?)
            }
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "send" => {
                let args: workspace::RequestArgs = parse_args(args)?;
                let request = self.prepare(&args)?;
                let result = if args.variables.is_empty() {
                    http::send(request, &self.bodies, ctx)
                } else {
                    http::send_with_redacted_urls(request, &self.bodies, ctx, true)
                };
                to_value(result.map_err(|error| args.protect_error(error))?)
            }
            "oauth_wait" => {
                let flow = parse_args::<oauth::FlowArgs>(args)?.flow;
                to_value(self.oauth.wait(flow, ctx)?)
            }
            "oauth_refresh" => {
                let args: RefreshArgs = parse_args(args)?;
                let key = oauth::session_key(&args.config.id, args.environment_id.as_deref());
                to_value(
                    self.oauth
                        .refresh(&args.config, &args.variables, &key, ctx)?,
                )
            }
            _ => self.call(function, args),
        }
    }
}

tf_plugin_guest::export_plugin!(HttpClient);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
