export interface Diag {
  code: string
  params?: Record<string, unknown>
}

export interface HeaderInput {
  name: string
  value: string
}

export interface HeaderView {
  name: string
  value: string
  masked: boolean
}

export interface Condition {
  id: string
  status: 'pass' | 'fail' | 'unknown'
  detail?: string | null
  reason?: Diag | null
}

export interface Stage {
  status: 'skipped' | 'allowed' | 'blocked' | 'undetermined'
  conditions: Condition[]
  reasons: Diag[]
}

export interface OptionsDraft {
  url: string
  headers: HeaderView[]
}

export interface RequestView {
  origin: string
  target: string
  method: string
  credentials: string
  sameOrigin: boolean
  preflightNeeded: boolean
  unsafeHeaderNames: string[]
  forbiddenHeaderNames: string[]
  options: OptionsDraft
}

export interface Reading {
  status: 'skipped' | 'allowed' | 'blocked' | 'undetermined'
  conditions: Condition[]
  reasons: Diag[]
  readable: string[]
  suppressed: Diag[]
}

export interface Report {
  request: RequestView
  preflight: Stage
  reading: Reading
  overall: 'allowed' | 'blocked' | 'undetermined'
  outcome: string
  diagnostics: Diag[]
}

export interface Exported {
  content: string
  ext: string
}
