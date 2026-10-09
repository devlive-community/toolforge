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
}

export interface Today {
  date: string
  year: number
  month: number
  holidayDataUntil: number
}
