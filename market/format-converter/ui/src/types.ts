export type Format = 'auto' | 'json' | 'json5' | 'yaml' | 'toml' | 'csv' | 'xml' | 'ini' | 'properties'
export type Concrete = Exclude<Format, 'auto'>
export type Delimiter = 'comma' | 'semicolon' | 'tab' | 'pipe'

export const FORMATS: Concrete[] = ['json', 'yaml', 'toml', 'csv', 'xml', 'ini', 'properties', 'json5']
/** JSON5 只作为输入，输出 JSON 即为合法 JSON5 */
export const OUTPUT_FORMATS: Concrete[] = FORMATS.filter((format) => format !== 'json5')
/** 读取时可推断数字 / 布尔类型的格式 */
export const INFERABLE: Concrete[] = ['csv', 'xml', 'ini', 'properties']

export interface Options {
  indent: number
  minify: boolean
  delimiter: Delimiter
  header: boolean
  inferTypes: boolean
}

export interface Warning {
  code: string
  path: string
}

export interface ConvertResult {
  output: string
  from: Concrete
  records: number | null
  warnings: Warning[]
  elapsedMs: number
}

export const EXTENSIONS: Record<Concrete, string[]> = {
  json: ['json'],
  yaml: ['yaml', 'yml'],
  toml: ['toml'],
  csv: ['csv', 'tsv', 'txt'],
  xml: ['xml', 'config', 'plist', 'xsd'],
  ini: ['ini', 'cfg', 'conf'],
  properties: ['properties'],
  json5: ['json5', 'jsonc'],
}

export const SAMPLES: Record<Concrete, string> = {
  json: `{
  "name": "ToolForge",
  "version": "0.1.2",
  "server": { "host": "127.0.0.1", "port": 8080 },
  "plugins": [
    { "id": "json-formatter", "enabled": true },
    { "id": "hash", "enabled": false }
  ]
}`,
  yaml: `name: ToolForge
version: 0.1.2
server:
  host: 127.0.0.1
  port: 8080
plugins:
  - id: json-formatter
    enabled: true
  - id: hash
    enabled: false
`,
  toml: `name = "ToolForge"
version = "0.1.2"
released = 2026-09-23T10:00:00Z

[server]
host = "127.0.0.1"
port = 8080

[[plugins]]
id = "json-formatter"
enabled = true
`,
  csv: `id,name,category,enabled,downloads
1,JSON Formatter,dev,true,1520
2,Hash,dev,true,980
3,Encoder,encode,false,310`,
  xml: `<?xml version="1.0" encoding="UTF-8"?>
<config version="0.1.2">
  <name>ToolForge</name>
  <server host="127.0.0.1" port="8080"/>
  <plugin id="json-formatter" enabled="true"/>
  <plugin id="hash" enabled="false"/>
</config>
`,
  ini: `; ToolForge
name = ToolForge
version = "0.1.2"

[server]
host = 127.0.0.1
port = 8080

[server.tls]
enabled = false
ciphers[] = TLS_AES_128_GCM_SHA256
ciphers[] = TLS_CHACHA20_POLY1305_SHA256
`,
  properties: `# ToolForge
app.name=ToolForge
app.version=0.1.2
server.host=127.0.0.1
server.port=8080
plugins[0].id=json-formatter
plugins[0].enabled=true
plugins[1].id=hash
plugins[1].enabled=false
`,
  json5: `// JSON5 允许注释、单引号与尾逗号
{
  name: 'ToolForge',
  version: '0.1.2',
  server: { host: '127.0.0.1', port: 8080, },
  plugins: [
    { id: 'json-formatter', enabled: true },
  ],
}
`,
}
