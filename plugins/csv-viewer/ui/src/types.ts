export type Kind = 'empty' | 'integer' | 'float' | 'boolean' | 'date' | 'text'

export interface Column {
  name: string | null
  kind: Kind
}

export interface TableInfo {
  id: number
  name: string | null
  path: string | null
  rows: number
  columns: Column[]
  delimiter: string
  encoding: string
  hasHeader: boolean
  bytes: number
  elapsedMs: number
}

export interface Row {
  index: number
  cells: string[]
}

export interface Page {
  total: number
  offset: number
  rows: Row[]
}

export type Op = 'contains' | 'equals' | 'notEquals' | 'greater' | 'less' | 'empty' | 'notEmpty'
export const OPS: Op[] = ['contains', 'equals', 'notEquals', 'greater', 'less', 'empty', 'notEmpty']
export const needsValue = (op: Op) => op !== 'empty' && op !== 'notEmpty'

export interface Filter {
  column: number
  op: Op
  value: string
}

export interface SortKey {
  column: number
  desc: boolean
}

export interface Spec {
  sort: SortKey[]
  query: string
  filters: Filter[]
}

export interface Stats {
  count: number
  empty: number
  distinct: number
  distinctCapped: boolean
  min: number | null
  max: number | null
  mean: number | null
  sum: number | null
  minLength: number | null
  maxLength: number | null
  top: { value: string; count: number }[]
}

export type Format = 'csv' | 'tsv' | 'json' | 'markdown'
