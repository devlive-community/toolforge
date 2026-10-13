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
