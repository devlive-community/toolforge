export interface Method {
  name: string
  path: string
  input: string
  output: string
  clientStreaming: boolean
  serverStreaming: boolean
  supported: boolean
}

export interface Service {
  name: string
  methods: Method[]
}

export interface Loaded {
  token: number
  services: Service[]
  files: number
  messages: number
}

export interface MetadataEntry {
  key: string
  value: string
  enabled: boolean
  secret: boolean
}

export interface Target {
  address: string
  tls: boolean
}

export interface Status {
  code: number
  name: string
  message: string
  local: boolean
}

export interface Batch {
  messages: string[]
  count: number
  bytes: number
  done: boolean
  status: Status | null
  trailers: [string, string][]
  elapsedMs: number
  truncated: boolean
}

export interface Started extends Batch {
  call: number
  headers: [string, string][]
}

export type SourceKind = 'proto' | 'descriptor' | 'reflection'

/** 保存的设置；metadata 中凭据的值由 Rust 清空后才写入 */
export interface Settings {
  target: Target
  deadlineSec: number
  metadata: MetadataEntry[]
  source: { kind: SourceKind; proto: string; includes: string[]; descriptor: string }
  method: string
  /** 各方法的请求草稿 */
  drafts: Record<string, string>
}

export const defaultSettings = (): Settings => ({
  target: { address: 'localhost:50051', tls: false },
  deadlineSec: 30,
  metadata: [],
  source: { kind: 'proto', proto: '', includes: [], descriptor: '' },
  method: '',
  drafts: {},
})

/** 调用的展示状态 */
export interface Run {
  call: number | null
  running: boolean
  headers: [string, string][]
  messages: string[]
  batch: Batch | null
}
