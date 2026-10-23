export type Level = 'none' | 'trace' | 'debug' | 'info' | 'warn' | 'error'
export const LEVELS: Level[] = ['error', 'warn', 'info', 'debug', 'trace', 'none']

export type Counts = Record<Level, number>

export interface Stats {
  lines: number
  bytes: number
  counts: Counts
}

export interface OpenInfo extends Stats {
  id: number
  name: string
  path: string
  encoding: string
  elapsedMs: number
}

/** 0–15 为终端基本色，字符串为 #rrggbb */
export type AnsiColor = number | string

export interface Span {
  start: number
  end: number
  fg?: AnsiColor
  bg?: AnsiColor
  bold?: boolean
  dim?: boolean
  italic?: boolean
  underline?: boolean
  strike?: boolean
  inverse?: boolean
}

export interface Line {
  n: number
  level: Level
  text: string
  truncated: boolean
  marks: [number, number][]
  styles: Span[]
}

export interface Exported {
  html: string
  text: string
  lines: number
}

export interface Page {
  total: number
  lines: Line[]
}

export interface Detail {
  n: number
  level: Level
  text: string
  truncated: boolean
  json: string | null
  styles: Span[]
}

export interface FilterResult {
  view: number
  total: number
  elapsedMs: number
}

export interface Refreshed {
  changed: boolean
  reset: boolean
  stats: Stats
  viewTotal: number | null
}
