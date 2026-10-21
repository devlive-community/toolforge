/** 家务轮值数据模型（与后端 serde 模型一一对应） */

export interface Member {
  id: string
  name: string
}

export type Rule =
  | { kind: 'daily' }
  | { kind: 'weekly'; weekdays: number[] }
  | { kind: 'interval'; days: number }

export type Mode =
  | { type: 'fixed'; memberId: string }
  | { type: 'rotation'; memberIds: string[] }

/** 纪元：一次规则 / 名单修改从某天起生效；历史纪元只读 */
export interface Epoch {
  from: string
  rule: Rule
  mode: Mode
}

export interface Chore {
  id: string
  name: string
  epochs: Epoch[]
  pausedFrom?: string
}

/** 一次 occurrence 的记录，键为 `choreId|YYYY-MM-DD` */
export interface OccurrenceRecord {
  status?: 'done' | 'skipped'
  executor?: string
  actualDate?: string
  assignee?: string
}

export interface Occurrence {
  key: string
  choreId: string
  choreName: string
  date: string
  assigneeId?: string
  assigneeName?: string
  status: 'pending' | 'done' | 'skipped'
  late: boolean
  missed: boolean
  overridden: boolean
}

export interface MemberStat {
  memberId: string
  name: string
  done: number
  skipped: number
  pending: number
}

export interface PlanOut {
  occurrences: Occurrence[]
  stats: MemberStat[]
}

export const MAX_MEMBERS = 20
export const MAX_CHORES = 50
export const MAX_INTERVAL_DAYS = 365
/** 今日看板回看逾期范围 */
export const OVERDUE_LOOKBACK_DAYS = 13

export const WEEKDAY_LABEL_KEYS = ['sun', 'mon', 'tue', 'wed', 'thu', 'fri', 'sat'] as const

/** 本地日期 → YYYY-MM-DD */
export function formatDate(date: Date): string {
  const y = date.getFullYear()
  const m = String(date.getMonth() + 1).padStart(2, '0')
  const d = String(date.getDate()).padStart(2, '0')
  return `${y}-${m}-${d}`
}

export function todayText(): string {
  return formatDate(new Date())
}

/** 本周周一（一周从周一开始） */
export function mondayOfWeek(): string {
  const now = new Date()
  const offset = (now.getDay() + 6) % 7
  const monday = new Date(now)
  monday.setDate(now.getDate() - offset)
  return formatDate(monday)
}

export function addDaysText(base: string, days: number): string {
  const [y, m, d] = base.split('-').map(Number)
  const date = new Date(y, m - 1, d)
  date.setDate(date.getDate() + days)
  return formatDate(date)
}

/** YYYY-MM-DD → 周几（0 = 周日） */
export function weekdayOf(text: string): number {
  const [y, m, d] = text.split('-').map(Number)
  return new Date(y, m - 1, d).getDay()
}
