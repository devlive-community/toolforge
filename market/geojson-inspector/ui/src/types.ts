export type Level = 'error' | 'warning' | 'hint'

export interface Diag {
  level: Level
  code: string
  params?: Record<string, unknown>
  pointer: string
  line?: number
  column?: number
}

export interface Stats {
  features: number
  positions: number
  types: Record<string, number>
  errors: number
  warnings: number
  hints: number
}

export interface Report {
  overall: 'passed' | 'invalid' | 'warnings' | 'incomplete'
  incomplete: boolean
  stats: Stats
  diagnostics: Diag[]
}

export interface Exported {
  content: string
  ext: string
}
