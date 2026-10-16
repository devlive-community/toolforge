export type Quote = 'none' | 'single' | 'double'

export type Line =
  | { kind: 'blank' }
  | { kind: 'comment'; text: string }
  | { kind: 'passthrough'; text: string }
  | {
      kind: 'entry'
      export: boolean
      key: string
      value: string
      quote: Quote
      inlineComment: string | null
      multiline: boolean
    }

export interface OpenResult {
  path: string
  lines: Line[]
}

export interface SaveResult {
  bytes: number
}

export interface ExampleResult {
  path: string
}

/** KEY 语法与后端 is_valid_key 一致 */
export const KEY_PATTERN = /^[A-Za-z_][A-Za-z0-9_.]*$/

export const ENV_EXTENSIONS = ['env', 'local', 'dev', 'prod', 'development', 'production', 'staging', 'test', 'example']
export const baseName = (path: string) => path.split(/[\\/]/).pop() ?? path

export type Issue = 'missing' | 'extra' | 'empty' | 'duplicate' | 'differs'

export interface Cell {
  state: 'missing' | 'empty' | 'set'
  duplicate: boolean
  /** 与基准值是否相同；基准自身或任一方缺失时为 null */
  sameAsBase: boolean | null
}

export interface Comparison {
  summaries: { entries: number; missing: number; extra: number; empty: number; duplicate: number; differs: number }[]
  rows: { key: string; cells: Cell[]; issues: Issue[] }[]
}

export interface MergeResult {
  text: string
  added: string[]
}
