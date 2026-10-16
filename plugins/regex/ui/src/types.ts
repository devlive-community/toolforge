export interface Flags {
  caseInsensitive: boolean
  multiLine: boolean
  dotAll: boolean
  extended: boolean
}

export const FLAG_KEYS: { key: keyof Flags; letter: string }[] = [
  { key: 'caseInsensitive', letter: 'i' },
  { key: 'multiLine', letter: 'm' },
  { key: 'dotAll', letter: 's' },
  { key: 'extended', letter: 'x' },
]

export interface Group {
  index: number
  name: string | null
  start: number
  end: number
  text: string
}

export interface Match {
  index: number
  start: number
  end: number
  text: string
  groups: (Group | null)[]
}

export interface TestResult {
  matches: Match[]
  count: number
  truncated: boolean
  groupNames: (string | null)[]
  elapsedMs: number
}

export interface ReplaceResult {
  output: string
  count: number
  elapsedMs: number
}

export interface BenchRun {
  length: number
  steps: number
  exceeded: boolean
  matched: boolean
  elapsedMs: number
}

export interface Finding {
  kind: 'nested_quantifier' | 'overlapping_alternation' | 'adjacent_quantifiers'
  severity: 'high' | 'medium'
  snippet: string
  attack: string
}

export interface AnalyzeResult {
  backtrackingEngine: boolean
  findings: Finding[]
  simulated: boolean
  sample: BenchRun | null
  sampleTruncated: boolean
  attacks: BenchRun[]
  growth: 'linear' | 'polynomial' | 'exponential' | null
  stepBudget: number
}
