export type Status = 'passed' | 'failure' | 'error' | 'skipped' | 'unknown'
export interface Counts {
  tests: number
  passed: number
  failures: number
  errors: number
  skipped: number
  unknown: number
  knownSeconds: number
  missingTime: number
}
export interface Summary {
  handle: string
  counts: Counts
  sourceCount: number
  suiteCount: number
  diagnosticCount: number
}
export interface Source {
  id: number
  path: string
  name: string
  counts: Counts
}
export interface Suite {
  id: number
  source: number
  parent: number | null
  name: string
  kind: string
  counts: Counts
  declared: { tests: number | null; failures: number | null; errors: number | null; skipped: number | null; seconds: number | null }
}
export interface Case {
  id: number
  source: number
  suite: number
  name: string
  classname: string
  status: Status
  seconds: number | null
  file: string | null
  line: string | null
}
export interface Diagnostic {
  source: number
  suite: number | null
  case: number | null
  kind: string
  detail: string
}
export interface Filter {
  source?: number
  suite?: number
  status?: Status
  query: string
  slowest: boolean
}
export interface Page<T> {
  items: T[]
  total: number
  counts?: Counts
}
export interface Block {
  id: number
  kind: string
  message: string | null
  typeName: string | null
  characters: number
}
export interface Detail {
  row: Case | Suite
  items: Block[]
  total: number
}
export interface Chunk {
  text: string
  offset: number
  limit: number
  total: number
}
