ToolForge 里的每个工具都是插件，包括随应用内置的工具。本文以在线插件为例，介绍从目录结构、后端、界面到命令行与 AI Agent 接口、构建与发布的完整流程。

> 本文面向在 [ToolForge 仓库](https://github.com/devlive-community/toolforge) 中开发插件的贡献者：在线插件放在 `market/<插件名>/`，由官方流水线签名发布。仓库外也可以打包插件，但只能生成未签名包，用户安装时需要手动确认信任。

## 插件由什么组成

一个插件包含四部分，最终打包成带 ed25519 签名的 `.tfp` 文件：

- **manifest.json**：插件的身份、权限、函数和接口声明。
- **Rust 后端**：编译为 `wasm32-wasip2` 组件，在 wasmtime 沙箱中运行。**所有数据处理都在这里完成**。
- **React 界面**：只负责输入和展示，不做数据处理。
- **locales**：中英文文案。界面、错误信息和接口说明都从这里取。

```text
market/my-tool/
├── manifest.json
├── icon.svg
├── locales/
│   ├── zh-CN.json
│   └── en-US.json
├── backend/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       └── lib_test.rs
└── ui/
    ├── package.json
    ├── tsconfig.json
    └── src/index.tsx
```

## 第一步：编写 manifest.json

```json
{
  "id": "org.devlive.toolforge.my-tool",
  "version": "0.1.0",
  "name": "i18n:name",
  "description": "i18n:description",
  "category": "dev",
  "keywords": ["example", "示例"],
  "accent": "blue",
  "locales": ["zh-CN", "en-US"],
  "minAppVersion": "0.2.0",
  "permissions": ["clipboard:write"],
  "functions": {
    "convert": { "history": true }
  },
  "sensitive": false
}
```

| 字段 | 说明 |
| --- | --- |
| `id` | 固定使用 `org.devlive.toolforge.<插件名>`，插件名用小写字母、数字和连字符 |
| `version` | 每次发布递增一次；同一个发布周期内多次修改共用一次递增 |
| `name` / `description` | 写成 `i18n:键名`，真实文案在 locales 中 |
| `category` | `dev`、`text`、`encode`、`convert`、`image`、`network`、`system`、`calc`、`life`、`other` 之一 |
| `minAppVersion` | 插件需要的最低应用版本，新插件和修改过的插件必须填写 |
| `permissions` | 插件代码需要的能力，由宿主强制检查，见下文 |
| `functions` | 后端提供的函数；`task: true` 表示耗时任务，`history: true` 表示调用成功后记入历史 |
| `sensitive` | 处理密钥、令牌等敏感数据时设为 `true`，输入不会写入历史 |

常用权限：

| 权限 | 能做什么 |
| --- | --- |
| `fs:user-selected` | 读写用户选择的文件（宿主函数 `read-file` / `write-file`） |
| `network` | 由宿主发送 HTTP 请求、监听一次性本地回调 |
| `clipboard:read` / `clipboard:write` | 界面读取或写入剪贴板 |

## 第二步：实现 Rust 后端

`backend/Cargo.toml`：

```toml
[package]
name = "tfm-my-tool"
version = "0.1.0"
edition.workspace = true

[lib]
# cdylib 编译为 WASM 组件，rlib 供本机单元测试
crate-type = ["cdylib", "rlib"]

[dependencies]
serde.workspace = true
serde_json.workspace = true
tf-plugin-api.workspace = true
tf-plugin-guest = { path = "../../../crates/tf-plugin-guest" }
```

`backend/src/lib.rs`：

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Args {
    input: String,
    #[serde(default)]
    uppercase: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Output {
    output: String,
    char_count: usize,
}

fn convert(args: Args) -> PluginResult<Output> {
    if args.input.is_empty() {
        // 只返回错误码和参数，文案由 locales 的 errors.<错误码> 翻译
        return Err(PluginError::new("my_tool.empty"));
    }
    let output = if args.uppercase { args.input.to_uppercase() } else { args.input.clone() };
    Ok(Output { char_count: output.chars().count(), output })
}

pub struct MyTool {
    manifest: Manifest,
}

impl Default for MyTool {
    fn default() -> Self {
        Self { manifest: Manifest::from_static(MANIFEST) }
    }
}

impl ToolPlugin for MyTool {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "convert" => to_value(convert(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(MyTool);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
```

几条必须遵守的约定：

- **输出结构加 `#[serde(rename_all = "camelCase")]`**，界面里的类型一律使用 camelCase，漏写会让字段静默对不上。
- **错误只返回错误码和参数**，例如 `PluginError::new("my_tool.too_large").with("mb", 64)`，在 locales 的 `errors` 中写对应文案。
- **测试放在独立的 `xxx_test.rs` 文件**，源文件末尾用 `#[path = "xxx_test.rs"]` 引入，不要在源文件里内联 `mod tests { … }`。
- **WASM 中不能开线程**，普通调用中也不能直接读文件；需要读文件的功能声明为任务（`task: true`），实现 `run_task`，通过 `TaskContext::open_file` 读取，并用 `log`、`progress`、`stage` 报告进度、用 `is_cancelled` 响应取消。

## 第三步：编写界面

`ui/src/index.tsx` 默认导出工具页面组件：

```tsx
import { useState } from 'react'
import { CodeEditor, Panel, Switch } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'

interface Output {
  output: string
  charCount: number
}

export default function MyTool() {
  const { t, errorMessage } = usePlugin()
  const [input, setInput] = useState('')
  const [uppercase, setUppercase] = useState(false)
  const { result, error } = useDebouncedCall<Output>('convert', { input, uppercase }, [input, uppercase])

  return (
    <div className="grid min-h-0 flex-1 grid-cols-2 gap-3">
      <Panel title={t('input')} actions={<Switch checked={uppercase} onCheckedChange={setUppercase} aria-label={t('uppercase')} />}>
        <CodeEditor value={input} onChange={setInput} />
      </Panel>
      <Panel title={t('output')}>
        {error ? <p className="p-3 text-danger">{errorMessage(error)}</p> : <CodeEditor value={result?.output ?? ''} readOnly />}
      </Panel>
    </div>
  )
}
```

界面约定：

- 只使用 `@toolforge/ui` 的组件和语义化的颜色 token，不使用原生 select、checkbox 等控件，也不写死颜色。
- 所有文案走 locales，不在界面里写死文字。
- 不使用 localStorage、IndexedDB 等浏览器存储；需要保存的状态用 `usePluginState`，由宿主存进 ToolForge 的 SQLite。
- 图标按钮必须带 Tooltip；Panel 正文默认没有内边距，放表单或文字时自己加（例如 `p-3`）。

## 第四步：开放给命令行与 AI Agent

新开发的插件需要为每个函数声明 CLI 与 AI Agent 接口；不适合开放的函数（依赖界面状态、需要打开系统对话框等）写 `"interface": false`。声明写在 manifest 的函数下面：

```json
"functions": {
  "convert": {
    "history": true,
    "interface": {
      "expose": ["cli", "agent"],
      "summary": "i18n:api.convert.summary",
      "effect": "read",
      "params": {
        "type": "object",
        "required": ["input"],
        "properties": {
          "input": { "type": "string", "description": "i18n:api.convert.input", "x-positional": 0, "x-stdin": true },
          "uppercase": { "type": "boolean", "description": "i18n:api.convert.uppercase", "default": false }
        }
      },
      "output": { "text": "/output" },
      "examples": ["echo hello | tfp-my-tool convert --uppercase"]
    }
  }
}
```

| 字段 | 说明 |
| --- | --- |
| `expose` | 开放给 `cli`、`agent` 中的哪些通道 |
| `effect` | `read`（只读计算）、`write`（写文件或数据）、`external`（网络、剪贴板等对外操作），决定 AI Agent 默认是否需要确认 |
| `uses` | 本函数用到的权限，必须是插件 `permissions` 的子集；有路径参数时必须包含 `fs:user-selected` |
| `params` | 一层扁平的 JSON Schema；路径参数写 `"format": "path"` 和 `"x-access": "read"` 或 `"write"` |
| `x-positional` / `x-stdin` | 命令行中作为位置参数、或在省略时从标准输入读取 |
| `x-sensitive` | 敏感参数，写入历史与日志时打码 |
| `output` | 命令行的文本输出：`{"text": "/指针"}` 或 `{"table": "/指针", "columns": [...]}` |

`summary` 和参数说明写在 locales 的 `api` 下，两种语言都要有。然后在后端测试中加一条检查，保证声明的参数能被后端接受：

```rust
#[test]
fn declared_interfaces_match_backend_args() {
    let checked = tf_plugin_api::interface::check_declared_interfaces(&MyTool::default()).unwrap();
    assert_eq!(checked, 1);
}
```

声明之后，用户可以在终端运行 `tfp-my-tool convert hello`，也可以让 AI Agent 通过 `tfp mcp` 调用；每个工具的开关和授权由用户在工具市场中调整。

## 第五步：构建、测试与本地安装

在仓库根目录执行：

```bash
# 后端单元测试
cargo test -p tfm-my-tool

# 约定检查：manifest、接口声明、文案是否齐全
cargo xtask rules

# 打包为未签名的 .tfp（默认输出到 dist/plugins）
cargo xtask plugin pack my-tool --unsigned
```

在 ToolForge 中打开「工具市场 → 在线插件」，点击「从文件安装…」，选择打包出的 `.tfp`。未签名的包会提示确认，确认后即可在侧栏中使用。

## 发布与版本

- 修改已发布的插件时递增一次 `version`；版本已经高于已发布版本时，不要重复递增。
- 用到较新的宿主能力时，相应提高 `minAppVersion`。
- 改动合入 `dev` 后，官方流水线会对新版本签名并发布到插件索引，用户在工具市场中即可看到更新。
