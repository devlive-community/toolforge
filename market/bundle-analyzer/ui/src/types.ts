export interface Totals {
  inputs: number
  outputs: number
  bytes: number
  sourceBytes: number
  attributed: number
  unattributed: number
  overAttributed: number
}
export interface Snapshot {
  name: string
  totals: Totals
  diagnostics: number
  firstOutput: string | null
}
export interface Summary {
  totalDelta: number | null
  handle: string
  before: Snapshot
  after: Snapshot | null
}
export type Side = 'before' | 'after'
export interface Pair {
  before: string
  after: string
}
export interface Output {
  path: string
  bytes: number
  entryPoint: string | null
  cssBundle: string | null
  modules: number
  imports: number
  attributed: number
  unattributed: number
  overAttributed: number
}
export interface Module {
  path: string
  bytes: number
  sourceBytes: number | null
  known: boolean
}
export interface Import {
  path: string
  kind: string
  external: boolean
  resolved: boolean
}
export interface Change {
  before: string | null
  after: string | null
  mode: string
  beforeBytes: number | null
  afterBytes: number | null
  delta: number
}
export interface ModuleChange {
  path: string
  beforeBytes: number | null
  afterBytes: number | null
  delta: number
}
export interface Diagnostic {
  kind: string
  path: string
  count: number
}
export interface Page<T> {
  items: T[]
  total: number
}
export interface Query {
  handle: string
  kind: string
  side?: Side
  output?: string | null
  other?: string | null
  module?: string | null
  search?: string
  pairs?: Pair[]
  directory?: string[]
  offset?: number
  limit?: number
}
export interface Tile {
  name: string
  kind: 'module' | 'directory' | 'other' | 'unattributed'
  bytes: number
  path: string | null
  directory: string[] | null
  count: number
  x: number
  y: number
  width: number
  height: number
}
export interface Tree {
  items: Tile[]
  basis: number
  groups: number
  limited: boolean
  depthLimited: boolean
  outputBytes: number
  attributed: number
  overAttributed: number
}
export interface Trace {
  state: 'found' | 'limited' | 'unavailable'
  root?: string
  module: string
  edges: { from: string; to: string; kind: string }[]
}
