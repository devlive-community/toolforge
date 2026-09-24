export type Field = 'second' | 'minute' | 'hour' | 'dayOfMonth' | 'month' | 'dayOfWeek' | 'year'

export type Part =
  | { kind: 'any' }
  | { kind: 'value'; value: number }
  | { kind: 'range'; from: number; to: number }
  | { kind: 'step'; from: number | null; to: number | null; step: number }
  | { kind: 'lastDay' }
  | { kind: 'lastWeekday' }
  | { kind: 'nearestWeekday'; day: number }
  | { kind: 'lastOfMonth'; weekday: number }
  | { kind: 'nth'; weekday: number; nth: number }

export interface FieldInfo {
  field: Field
  raw: string
  parts: Part[]
}

export interface Run {
  local: string
  iso: string
  weekday: number
  inSeconds: number
}

export interface Report {
  normalized: string
  timezone: string
  hasSeconds: boolean
  fields: FieldInfo[]
  next: Run[]
  previous: Run | null
}
