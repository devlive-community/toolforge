export type Level = 'error' | 'warning' | 'hint'

export interface Diag {
  level: Level
  code: string
  params?: Record<string, unknown>
  offset?: number
}

export interface Entry {
  key: Node
  value: Node
}

export interface Node {
  kind: string
  offset: number
  length: number
  value?: string | null
  hex?: string | null
  hexTruncated?: boolean | null
  children?: Node[] | null
  entries?: Entry[] | null
  number?: number | null
  indefinite?: boolean | null
}

export interface Trailing {
  offset: number
  length: number
}

export interface Report {
  format: 'cbor' | 'msgpack'
  overall: 'passed' | 'issues' | 'incomplete'
  incomplete: boolean
  stats: {
    nodes: number
    errors: number
    warnings: number
    hints: number
  }
  root?: Node | null
  trailing?: Trailing | null
  diagnostics: Diag[]
}

export interface Exported {
  content: string
  ext: string
}
