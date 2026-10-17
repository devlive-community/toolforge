export type Format = 'auto' | 'csv' | 'tsv' | 'json' | 'jsonl'
export type HeaderMode = 'auto' | 'yes' | 'no'
export type Kind = 'number' | 'time' | 'text'
export type Chart = 'line' | 'bar' | 'scatter'
export type XType = 'auto' | 'number' | 'time' | 'category'
export type Side = 'left' | 'right'

export interface Note {
  code: string
  params: Record<string, unknown>
}

export interface ColumnInfo {
  index: number
  name: string
  kind: Kind
  present: number
  min: number | null
  max: number | null
}

export interface Summary {
  token: number
  format: Exclude<Format, 'auto'>
  delimiter: 'comma' | 'tab' | 'semicolon' | 'pipe' | 'whitespace' | null
  header: boolean
  jsonPath: string | null
  rows: number
  columns: ColumnInfo[]
  preview: string[][]
  notes: Note[]
  suggestion: { chart: Chart; x: number | null; series: number[] }
  file?: string
}

export interface SeriesSpec {
  column: number
  axis: Side
}

export interface Config {
  chart: Chart
  x: number | null
  xType: XType
  series: SeriesSpec[]
  logLeft: boolean
  logRight: boolean
}

export interface Tick {
  value: number
  label: string
}

export interface Scale {
  min: number
  max: number
  ticks: Tick[]
}

export interface XAxis extends Scale {
  kind: 'index' | 'number' | 'time' | 'category'
  name: string
}

export interface YAxis extends Scale {
  log: boolean
  names: string[]
}

export interface Series {
  name: string
  column: number
  axis: Side
  points: [number, number][]
  total: number
  skipped: number
  sampled: boolean
}

export interface Plot {
  chart: Chart
  x: XAxis
  left: YAxis | null
  right: YAxis | null
  series: Series[]
  categories?: string[]
  notes: Note[]
}

/** 系列颜色：导出的图片里也要用具体颜色，所以不用主题变量 */
export const PALETTE = ['#2563eb', '#16a34a', '#ea580c', '#9333ea', '#dc2626', '#0891b2', '#ca8a04', '#db2777', '#4f46e5', '#65a30d', '#0d9488', '#7c3aed']
