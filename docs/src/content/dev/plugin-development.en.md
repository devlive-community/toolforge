Every tool in ToolForge is a plugin, including the ones built into the app. This article walks through an online plugin from start to finish: layout, backend, user interface, the command line and AI agent interface, and building and publishing.

> This guide is for contributors developing plugins in the [ToolForge repository](https://github.com/devlive-community/toolforge). Online plugins live in `market/<plugin>/` and are signed and published by the official pipeline. You can package a plugin outside the repository too, but only as an unsigned package that users must explicitly trust when installing.

## What a plugin is made of

A plugin has four parts and ships as an ed25519-signed `.tfp` file:

- **manifest.json**: identity, permissions, functions and interface declarations.
- **Rust backend**: compiled to a `wasm32-wasip2` component and run in the wasmtime sandbox. **All data processing happens here.**
- **React user interface**: input and display only, no data processing.
- **locales**: Chinese and English text for the interface, error messages and interface docs.

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

## Step 1: Write manifest.json

```json
{
  "id": "org.devlive.toolforge.my-tool",
  "version": "0.1.0",
  "name": "i18n:name",
  "description": "i18n:description",
  "category": "dev",
  "keywords": ["example"],
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

| Field | Meaning |
| --- | --- |
| `id` | Always `org.devlive.toolforge.<plugin>`, using lowercase letters, digits and hyphens |
| `version` | Bump once per release; several changes in one release cycle share one bump |
| `name` / `description` | Written as `i18n:key`; the real text lives in the locales |
| `category` | One of `dev`, `text`, `encode`, `convert`, `image`, `network`, `system`, `calc`, `other` |
| `minAppVersion` | The lowest app version the plugin needs; required for new and changed plugins |
| `permissions` | Capabilities the plugin code needs, enforced by the host (see below) |
| `functions` | Functions the backend provides; `task: true` marks long-running tasks, `history: true` records successful calls in history |
| `sensitive` | Set to `true` when handling secrets such as keys or tokens; inputs are never written to history |

Common permissions:

| Permission | Allows |
| --- | --- |
| `fs:user-selected` | Reading and writing files the user chose (host functions `read-file` / `write-file`) |
| `network` | HTTP requests sent by the host and one-off local callbacks |
| `clipboard:read` / `clipboard:write` | Reading or writing the clipboard from the interface |

## Step 2: Implement the Rust backend

`backend/Cargo.toml`:

```toml
[package]
name = "tfm-my-tool"
version = "0.1.0"
edition.workspace = true

[lib]
# cdylib builds the WASM component, rlib runs native unit tests
crate-type = ["cdylib", "rlib"]

[dependencies]
serde.workspace = true
serde_json.workspace = true
tf-plugin-api.workspace = true
tf-plugin-guest = { path = "../../../crates/tf-plugin-guest" }
```

`backend/src/lib.rs`:

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
        // Return an error code with parameters only; locales translate errors.<code>
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

Rules to follow:

- **Add `#[serde(rename_all = "camelCase")]` to output structs.** The interface uses camelCase everywhere, and a missing attribute silently breaks field names.
- **Errors carry only a code and parameters**, such as `PluginError::new("my_tool.too_large").with("mb", 64)`; write the message under `errors` in the locales.
- **Tests live in separate `xxx_test.rs` files**, included with `#[path = "xxx_test.rs"]` at the end of the source file, never as an inline `mod tests { … }`.
- **WASM cannot start threads**, and plain calls cannot read files directly. Declare file-reading work as a task (`task: true`), implement `run_task`, read through `TaskContext::open_file`, report with `log`, `progress` and `stage`, and stop when `is_cancelled` returns true.

## Step 3: Build the interface

`ui/src/index.tsx` exports the tool page as its default export:

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

Interface rules:

- Use components from `@toolforge/ui` and semantic color tokens only; no native select or checkbox controls and no hard-coded colors.
- All text comes from the locales; never hard-code strings in the interface.
- Do not use localStorage, IndexedDB or other browser storage. Persist state with `usePluginState`, which the host stores in the ToolForge SQLite database.
- Icon buttons need a Tooltip. Panel bodies have no padding by default, so add it yourself (for example `p-3`) for forms or text.

## Step 4: Open it to the command line and AI agents

New plugins declare a CLI and AI agent interface for every function. Functions that are not suitable (they depend on interface state or need a system dialog) use `"interface": false`. The declaration sits under the function in the manifest:

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

| Field | Meaning |
| --- | --- |
| `expose` | Which channels get the function: `cli`, `agent` |
| `effect` | `read` (computation only), `write` (writes files or data) or `external` (network, clipboard and other outside effects); decides whether AI agents ask for confirmation by default |
| `uses` | Permissions this function uses, a subset of the plugin `permissions`; path parameters require `fs:user-selected` |
| `params` | A flat JSON Schema; path parameters add `"format": "path"` and `"x-access": "read"` or `"write"` |
| `x-positional` / `x-stdin` | A positional argument on the command line, or read from standard input when omitted |
| `x-sensitive` | A sensitive argument, masked in history and logs |
| `output` | Command line text output: `{"text": "/pointer"}` or `{"table": "/pointer", "columns": [...]}` |

Write `summary` and the parameter descriptions under `api` in both locales. Then add a backend test that makes sure the backend accepts the declared parameters:

```rust
#[test]
fn declared_interfaces_match_backend_args() {
    let checked = tf_plugin_api::interface::check_declared_interfaces(&MyTool::default()).unwrap();
    assert_eq!(checked, 1);
}
```

Once declared, users can run `tfp-my-tool convert hello` in a terminal, and AI agents can call it through `tfp mcp`. Users turn each tool on or off for the command line and agents from the tool market.

## Step 5: Build, test and install locally

From the repository root:

```bash
# Backend unit tests
cargo test -p tfm-my-tool

# Convention checks: manifest, interface declarations and locales
cargo xtask rules

# Package an unsigned .tfp (written to dist/plugins by default)
cargo xtask plugin pack my-tool --unsigned
```

In ToolForge, open Tool market → Online plugins, choose "Install from file…" and pick the packaged `.tfp`. Unsigned packages ask for confirmation; after that the tool appears in the sidebar.

## Publishing and versions

- Bump `version` once when changing a published plugin; do not bump again while the version is already above the published one.
- Raise `minAppVersion` when the plugin relies on newer host capabilities.
- After the change is merged into `dev`, the official pipeline signs the new version and publishes it to the plugin index, and users see the update in the tool market.
