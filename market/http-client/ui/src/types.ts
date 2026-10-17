export interface Pair {
  key: string
  value: string
  enabled: boolean
}

export type BodyKind = 'none' | 'json' | 'text' | 'form'

export const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'] as const
export type Method = (typeof METHODS)[number]

export interface Request {
  method: Method
  url: string
  params: Pair[]
  headers: Pair[]
  body: { kind: BodyKind; text: string; form: Pair[] }
  timeoutMs: number
  followRedirects: boolean
  useProxy: boolean
  /** 流式接收（SSE 与其他分块响应）；旧的保存数据没有这个字段 */
  stream?: boolean
}

export interface Variable extends Pair {
  secret: boolean
}

/** 集合默认使用的授权配置 id */
export interface Collection { id: string; name: string; auth?: string | null }
/** auth：未设置时沿用集合；'none' 不使用授权；否则为授权配置 id */
export interface SavedRequest { id: string; name: string; collectionId: string | null; request: Request; auth?: string | null }

export const AUTH_NONE = 'none'

/** OAuth 授权配置：只含非敏感信息，随工作区保存 */
export interface OAuthConfig {
  id: string
  name: string
  authorizationUrl: string
  tokenUrl: string
  clientId: string
  scope: string
  redirectPort: number | null
  redirectPath: string
}

/** 后端内存中的授权会话（令牌已遮罩） */
export interface SessionView {
  key: string
  tokenType: string
  scope: string | null
  expiresAt: number | null
  obtainedAt: number
  refreshedAt: number | null
  expired: boolean
  hasRefreshToken: boolean
  rotated: boolean
  masked: string
  lastError: string | null
}

export interface OAuthFlow {
  flow: number
  key: string
  configId: string
  authorizeUrl: string
  redirectUri: string
  timeoutSecs: number
}
export interface Environment { id: string; name: string; variables: Variable[] }
export interface Workspace {
  version: 1
  collections: Collection[]
  requests: SavedRequest[]
  environments: Environment[]
  activeEnvironmentId: string | null
  oauthConfigs?: OAuthConfig[]
}

export interface CurlWarning { code: string; params: Record<string, unknown> }
export interface ImportedCurl { request: Request; warnings: CurlWarning[] }

export const newRequest = (): Request => ({
  method: 'GET',
  url: 'https://api.github.com/repos/devlive-community/toolforge',
  params: [],
  headers: [{ key: 'Accept', value: 'application/json', enabled: true }],
  body: { kind: 'none', text: '{\n  \n}', form: [] },
  timeoutMs: 30_000,
  followRedirects: true,
  useProxy: true,
  stream: false,
})

export interface Reply {
  id: number
  status: number
  statusText: string
  version: string
  url: string
  elapsedMs: number
  size: number
  headers: [string, string][]
  contentType: string | null
  kind: 'json' | 'text' | 'binary'
  body: string
  truncated: boolean
  redirects: string[]
}

export const emptyPair = (): Pair => ({ key: '', value: '', enabled: true })

export interface StreamItem {
  seq: number
  atMs: number
  kind: 'event' | 'chunk'
  event?: string
  id?: string
  data: string
  /** data 是 JSON 时后端给出的缩进展示 */
  formatted?: string
  retry?: number
  truncated: boolean
}

export interface StreamEnd {
  reason: 'ended' | 'stopped' | 'failed'
  error?: { code: string; params?: Record<string, unknown> }
  discarded: boolean
}

export interface StreamBatch {
  events: StreamItem[]
  total: number
  dropped: number
  bytes: number
  comments: number
  retry: number | null
  lastId: string | null
  elapsedMs: number
  done: StreamEnd | null
}

export interface StreamOpened extends StreamBatch {
  session: number
  status: number
  statusText: string
  version: string
  headers: [string, string][]
  contentType: string | null
  mode: 'sse' | 'raw'
}

/** 前端最多保留的事件数，与后端一致 */
export const MAX_STREAM_EVENTS = 5000
