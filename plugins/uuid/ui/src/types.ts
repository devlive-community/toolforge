export type Kind = 'v4' | 'v7' | 'v1' | 'v5' | 'v3' | 'ulid' | 'nanoid'
export type Namespace = 'dns' | 'url' | 'oid' | 'x500' | 'custom'

export const KINDS: Kind[] = ['v4', 'v7', 'v1', 'v5', 'v3', 'ulid', 'nanoid']
export const NAMESPACES: Namespace[] = ['dns', 'url', 'oid', 'x500', 'custom']

export const isUuid = (kind: Kind) => kind.startsWith('v')
export const isNamed = (kind: Kind) => kind === 'v3' || kind === 'v5'

export interface GenerateResult {
  ids: string[]
  deterministic: boolean
  elapsedMs: number
}

export interface InspectResult {
  kind: 'uuid' | 'ulid'
  canonical: string
  version: number | null
  variant: string | null
  timestamp: string | null
  unixMs: number | null
  hex: string
  isNil: boolean
  isMax: boolean
}
