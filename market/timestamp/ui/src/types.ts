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
  format: 'rfc3339' | 'zoned' | 'rfc2822' | 'datetime' | 'date' | 'natural'
  /** 自然语言的全部解释 */
  candidates: { datetime: string; weekday: number; notes: string[] }[]
  choice: number
  zoned: ZonedRow
}

export interface Zones {
  local: string
  all: string[]
}

// ---- 约时间（跨时区会议时段规划）----

/** 每周可用时段；end < start 表示跨午夜 */
export interface Slot {
  day: number
  start: string
  end: string
  choices: { date: string; offsetSeconds: number }[]
}

export interface PlannerParticipant {
  id: string
  label: string
  timezone: string
  slots: Slot[]
}

export interface PlanArgs {
  participants: { label: string; timezone: string; slots: Slot[] }[]
  baseTimezone: string
  startDate: string
  durationMinutes: number
  searchDays: number
}

export interface PlanRow {
  label: string
  timezone: string
  date: string
  weekday: number
  startMinute: number
  endDate: string
  endMinute: number
  crossDay: boolean
}

export interface PlanWindow {
  startMinute: number
  endMinute: number
  durationMinutes: number
  utcStart: string
  utcEnd: string
  rows: PlanRow[]
}

export interface PlanDay {
  date: string
  weekday: number
  windows: PlanWindow[]
}

export interface PlanOutput {
  baseTimezone: string
  days: PlanDay[]
  gaps: { label: string; timezone: string; date: string; time: string; participantIndex: number; slotIndex: number }[]
  ambiguities: {
    label: string
    timezone: string
    date: string
    time: string
    earlierOffsetSeconds: number
    earlierOffset: string
    laterOffsetSeconds: number
    laterOffset: string
    participantIndex: number
    slotIndex: number
  }[]
  text: string
}
