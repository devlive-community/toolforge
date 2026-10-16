export interface Counts {
  found: number
  hit: number
}

export interface Metrics {
  lines: Counts
  functions: Counts
  branches: Counts
  branchesNotExecuted: number
}

export interface Entry extends Metrics {
  path: string
  files: number
}

export interface Diagnostic {
  kind: 'malformed' | 'outsideRecord' | 'unterminated' | 'declaredMismatch' | 'duplicateRecord'
  line?: number
  file?: string
  detail?: string
}

export interface Overview {
  token: number
  version: string | null
  tests: string[]
  records: number
  totals: Metrics
  files: Entry[]
  dirs: Entry[]
  commonPrefix: string
  diagnostics: Diagnostic[]
  diagnosticsDropped: number
  ignored: Record<string, number>
}

export interface LineView {
  line: number
  text?: string
  hits: number | null
  branches?: [number, number]
}

export interface Detail {
  path: string
  metrics: Metrics
  tests: string[]
  lines: LineView[]
  functions: { name: string; line: number | null; endLine: number | null; hits: number }[]
  branches: { line: number; block: string; branch: string; taken: number | null }[]
  uncovered: { start: number; end: number }[]
  sourceIssues: { kind: 'beyondSource' | 'checksumMismatch'; line: number }[]
  sourceIssueCount: number
}

export interface Mapping {
  from: string
  to: string
}

/** 覆盖率百分比；分母为 0 时为 null（显示 N/A） */
export const ratio = (counts: Counts) => (counts.found === 0 ? null : (counts.hit / counts.found) * 100)
