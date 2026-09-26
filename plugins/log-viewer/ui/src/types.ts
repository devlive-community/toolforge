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

export interface Line {
  n: number
  level: Level
  text: string
  truncated: boolean
  marks: [number, number][]
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
