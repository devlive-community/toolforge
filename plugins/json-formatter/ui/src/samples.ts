import type { Mode } from './types'

/** 「示例」菜单中的静态示例文本 */
export const SAMPLES: { key: string; mode: Mode; text: string }[] = [
  {
    key: 'basic',
    mode: 'json',
    text: `{
  "name": "ToolForge",
  "version": "0.1.0",
  "description": "现代化桌面工具应用",
  "tech": {
    "frontend": "Tauri",
    "backend": "Rust"
  },
  "features": [
    {"name": "JSON 格式化", "icon": "{}"},
    {"name": "文本处理", "icon": "T"},
    {"name": "编码/加密", "icon": "🔒"},
    {"name": "格式转换", "icon": "🔁"},
    {"name": "网络工具", "icon": "🌐"},
    {"name": "系统工具", "icon": "🖥️"}
  ],
  "platform": ["Windows", "macOS", "Linux"],
  "license": "MIT",
  "website": "https://toolforge.dev",
  "tags": ["developer", "tools", "productivity"],
  "created_at": "2026-09-23T10:00:00Z"
}`,
  },
  {
    key: 'array',
    mode: 'json',
    text: `[{"id":1,"user":"alice","active":true,"score":98.5},{"id":2,"user":"bob","active":false,"score":null},{"id":3,"user":"carol","active":true,"score":87}]`,
  },
  {
    key: 'nested',
    mode: 'json',
    text: `{"order":{"id":"A-1024","customer":{"name":"Li Lei","address":{"city":"Shanghai","zip":"200000"}},"items":[{"sku":"KB-01","qty":1,"price":399},{"sku":"MS-02","qty":2,"price":129}],"paid":true}}`,
  },
  {
    key: 'json5',
    mode: 'json5',
    text: `{
  // JSON5 允许注释、单引号、无引号键与尾逗号
  name: 'ToolForge',
  ports: [1420, 1421,],
  debug: true,
}`,
  },
  {
    key: 'invalid',
    mode: 'json',
    text: `{
  "name": "ToolForge",
  "version": "0.1.0",
  "tags": ["a", "b",],
}`,
  },
]
