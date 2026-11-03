export type Level = 'error' | 'warning' | 'note' | 'none'
export interface Counts {
  results: number
  errors: number
  warnings: number
  notes: number
  none: number
  suppressed: number
}
export interface Summary {
  handle: string
  counts: Counts
  fileCount: number
  runCount: number
  ruleCount: number
  artifactCount: number
  diagnosticCount: number
}
export interface RunRow {
  id: number
  file: number
  tool: string
  version: string | null
  columnKind: string
  artifactCount: number
  ruleCount: number
  resultCount: number
  extensionCount: number
  externalProperties: boolean
  invocationSuccess: boolean | null
  automationId: string | null
}
export interface RuleRow {
  id: number
  run: number
  ruleId: string
  name: string | null
  shortDescription: string | null
  defaultLevel: Level | null
  helpUri: string | null
}
export interface ArtifactRow {
  id: number
  run: number
  uri: string
  uriBaseId: string | null
  language: string | null
  roles: string[]
}
export interface Region {
  startLine: number | null
  startColumn: number | null
  endLine: number | null
  endColumn: number | null
}
export interface LocationRow {
  id: number
  run: number
  result: number
  kind: 'physical' | 'related' | 'step'
  artifact: number | null
  uri: string | null
  uriBaseId: string | null
  region: Region | null
  snippet: string | null
  logical: string | null
  message: string | null
  order: number | null
  nesting: number | null
}
export interface Suppression {
  kind: string
  status: string | null
  justification: string | null
}
export interface ResultRow {
  id: number
  run: number
  rule: number | null
  ruleId: string
  level: Level
  kind: string
  message: string
  baselineState: string | null
  rank: number | null
  suppressed: boolean
  suppressions: Suppression[]
  occurrenceCount: number | null
}
export interface Diagnostic {
  run: number
  result: number | null
  kind: string
  detail: string
}
export interface Filter {
  run?: number
  level?: Level
  ruleId?: string
  artifact?: number
  suppressed?: boolean
  baseline?: string
  query: string
}
export interface Page<T> {
  items: T[]
  total: number
  counts?: Counts
}
export interface Detail {
  row: ResultRow
  rule: RuleRow | null
  columnKind: string
  locations: LocationRow[]
  related: LocationRow[]
  flows: { id: number; message: string | null; steps: LocationRow[] }[]
  diagnostics: Diagnostic[]
}
export interface SourceLine {
  no: number
  text: string
}
export interface SourceChunk {
  uri: string
  resolved: string
  columnKind: string
  region: Region
  lines: SourceLine[]
  truncated: boolean
  notes: string[]
}
