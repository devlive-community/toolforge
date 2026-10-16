export type Mode = 'lines' | 'csv'
export type Status = 'valid' | 'unconfirmed' | 'invalid'

export type Problem =
  | { kind: 'length'; found: number }
  | { kind: 'char'; position: number; found: string }
  | { kind: 'check'; expected: string; found: string }

export interface Field {
  code: string
  name: string | null
}

export interface Row {
  line: number
  raw: string
  normalized: { value: string; trimmed: boolean; removedSpaces: boolean; uppercased: boolean; fullwidth: boolean }
  status: Status
  problems: Problem[]
  fields: {
    department: Field
    category: Field
    region: Field
    subject: string
    orgCheckOk: boolean
    orgCheckExpected: string | null
    check: string
  } | null
}

export interface Output {
  rules: string
  rows: Row[]
  summary: { valid: number; unconfirmed: number; invalid: number; empty: number }
  columns: string[]
}

export const SAMPLE = `91350100M000100Y43
91350100M000100Y44
 91350100m000100y43
9135010OM000100Y43
B1350100MA00010002`
