export type Unit = 'auto' | 's' | 'ms' | 'us' | 'ns'
export const UNITS: Unit[] = ['auto', 's', 'ms', 'us', 'ns']

export interface ZonedRow {
  timezone: string
  datetime: string
  offset: string
  abbreviation: string
}

export interface Now {
  seconds: number
  milliseconds: number
  iso: string
  local: ZonedRow
}

export interface Converted {
  unit: Exclude<Unit, 'auto'>
  seconds: number
  milliseconds: number
  iso: string
  rfc2822: string
  weekday: number
  dayOfYear: number
  isoWeek: number
  relativeSeconds: number
  rows: ZonedRow[]
}

export interface Parsed {
  seconds: number
  milliseconds: number
  microseconds: number
  nanoseconds: string
  iso: string
  format: 'rfc3339' | 'zoned' | 'rfc2822' | 'datetime' | 'date'
  zoned: ZonedRow
}

export interface Zones {
  local: string
  all: string[]
}
