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
}

export interface Variable extends Pair {
  secret: boolean
}

export interface Collection { id: string; name: string }
export interface SavedRequest { id: string; name: string; collectionId: string | null; request: Request }
export interface Environment { id: string; name: string; variables: Variable[] }
export interface Workspace {
  version: 1
  collections: Collection[]
  requests: SavedRequest[]
  environments: Environment[]
  activeEnvironmentId: string | null
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
