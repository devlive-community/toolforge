import type { AppError } from '@toolforge/plugin-ui-sdk'

export type Order = 'asc' | 'desc'

export type Outcome = { kind: 'match' } | { kind: 'failed'; comparator: string } | { kind: 'prerelease' }

export interface Row {
  line: number
  input: string
  normalized: string | null
  major: number | null
  minor: number | null
  patch: number | null
  prerelease: (string | number)[]
  build: string[]
  error: AppError | null
  outcome: Outcome | null
}

export interface Analysis {
  rows: Row[]
  valid: number
  invalid: number
  matched: number | null
  sets: string[]
  rangeError: AppError | null
  equalPrecedence: [number, number][]
  csv: string
}

export const SAMPLE_VERSIONS = `0.2.3
0.2.9
0.3.0
1.0.0-beta.2
1.0.0-beta.11
1.0.0-rc.1
1.0.0
1.0.0+build.7
1.2.3
2.0.0-alpha
2.0.0`

export const PRESETS = ['^0.2.3', '~1.2', '>=1.0.0 <2.0.0', '1.x || >=2.0.0-alpha', '1.0.0 - 1.2', '*']
