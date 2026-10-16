export interface Holiday {
  index: number
  work: boolean
}

export interface Workdays {
  weekdays: number
  china: number
  holidays: number
  adjusted: number
  beyondData: boolean
  dataUntil: number
  custom: { workdays: number; restDays: number; workWeekends: number; beyondData: boolean } | null
}

export interface Difference {
  negative: boolean
  years: number
  months: number
  days: number
  hours: number
  minutes: number
  seconds: number
  totalDays: number
  weeks: number
  weekDays: number
  totalHours: number
  totalMinutes: number
  totalSeconds: number
  hasTime: boolean
  workdays: Workdays | null
}

export interface Added {
  result: string
  weekday: number
  clamped: boolean
}

export interface Info {
  date: string
  weekday: number
  isoYear: number
  isoWeek: number
  dayOfYear: number
  daysInYear: number
  quarter: number
  leapYear: boolean
  fromToday: number
  constellation: number
  lunar: { text: string; year: string; zodiac: number; month: string; day: string; leap: boolean } | null
  term: { index: number; day: number } | null
  festivals: string[]
  holiday: Holiday | null
  pillars: [string, string, string] | null
  phase: number | null
  recommends: string[]
  avoids: string[]
}

export interface Cell {
  date: string
  day: number
  inMonth: boolean
  today: boolean
  weekend: boolean
  label: string
  special: boolean
  holiday: Holiday | null
  custom: CustomDay | null
  workday: boolean | null
}

export interface Today {
  date: string
  year: number
  month: number
  holidayDataUntil: number
}

export interface LunarDate {
  year: number
  month: number
  day: number
  leap: boolean
}

export interface ConstellationRange {
  index: number
  start: [number, number]
  end: [number, number]
}

export interface Birth {
  date: string
  lunar: string
  zodiacLunarYear: number
  lunarYearGanzhi: string
  zodiacLichun: number
  lichunYearGanzhi: string
  lichun: string
  springFestival: string
  constellation: number
  constellations: ConstellationRange[]
}

export interface Anniversary {
  name: string
  month: number
  day: number
  leap: boolean
}

export interface Upcoming {
  date: string
  lunar: string
  days: number
  adjusted: 'noLeap' | 'lastDay' | null
}

export type Base = 'china' | 'weekdays'

export interface CustomDay {
  date: string
  work: boolean
  name?: string
}

export interface Scheme {
  name: string
  base: Base
  /** 周末：1 = 周一 … 7 = 周日 */
  weekend: number[]
  days: CustomDay[]
}

/** 保存在插件状态中的方案（带本地 id） */
export interface StoredScheme extends Scheme {
  id: string
}

export const toScheme = ({ id: _id, ...scheme }: StoredScheme): Scheme => scheme
