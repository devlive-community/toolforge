export type Mode = 'lines' | 'words' | 'chars'
export type Algo = 'myers' | 'patience'
export type Tag = 'equal' | 'delete' | 'insert' | 'change'

export interface Segment {
  text: string
  emphasized: boolean
}

export interface Line {
  number: number
  segments: Segment[]
}

export interface Row {
  tag: Tag
  left: Line | null
  right: Line | null
}

export interface DiffResult {
  mode: Mode
  identical: boolean
  rows: Row[]
  tokens: { tag: Tag; text: string }[]
  truncated: boolean
  stats: { added: number; removed: number; changed: number; unchanged: number; similarity: number }
  unified: string
  elapsedMs: number
}

export const LEFT_SAMPLE = `[server]
host = "127.0.0.1"
port = 8080
workers = 4

[database]
url = "postgres://localhost/toolforge"
pool = 10
timeout = 30`

export const RIGHT_SAMPLE = `[server]
host = "0.0.0.0"
port = 8080
workers = 8
keep_alive = true

[database]
url = "postgres://db.internal/toolforge"
pool = 10`
