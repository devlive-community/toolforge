export interface RegValue {
  kind: string
  text?: string
  items?: string[]
  value?: number
  bytes?: number[]
  typeId?: number
}

export interface ValueEntry {
  name: string
  isDefault: boolean
  value: RegValue
  line: number
  duplicate: boolean
  issues: string[]
}

export interface KeyNode {
  path: string
  segments: string[]
  values: ValueEntry[]
  line: number
  delete: boolean
  duplicate: boolean
}

export interface RegDoc {
  keys: KeyNode[]
  issues: string[]
}

export interface ParseOut {
  doc: RegDoc
  valueCount: number
}

export type DiffKind = 'keyAdded' | 'keyMissing' | 'valueAdded' | 'valueMissing' | 'typeChanged' | 'dataChanged'

export interface DiffEntry {
  kind: DiffKind
  path: string
  valueName: string | null
  a: string | null
  b: string | null
  lineA: number | null
  lineB: number | null
}

export interface DiffReport {
  entries: DiffEntry[]
  keysA: number
  keysB: number
  identical: boolean
  hasDeleteStatements: boolean
}

export interface DiffOut {
  report: DiffReport
  docA: RegDoc
  docB: RegDoc
}
