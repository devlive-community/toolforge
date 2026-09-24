export type Format = 'auto' | 'json' | 'yaml' | 'toml' | 'csv'
export type Concrete = Exclude<Format, 'auto'>
export type Delimiter = 'comma' | 'semicolon' | 'tab' | 'pipe'

export const FORMATS: Concrete[] = ['json', 'yaml', 'toml', 'csv']

export interface Options {
  indent: number
  minify: boolean
  delimiter: Delimiter
  header: boolean
  inferTypes: boolean
}

export interface ConvertResult {
  output: string
  from: Concrete
  records: number | null
  elapsedMs: number
}

export const EXTENSIONS: Record<Concrete, string[]> = {
  json: ['json'],
  yaml: ['yaml', 'yml'],
  toml: ['toml'],
  csv: ['csv', 'tsv', 'txt'],
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
}
