export interface Pair {
  key: string
  value: string
  enabled: boolean
}

export type BodyKind = 'none' | 'json' | 'text' | 'form'

export const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'] as const
export type Method = (typeof METHODS)[number]

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
