export interface Totals {
  keys: number
  clicks: number
  scroll: number
  /** 毫米 */
  distance: number
  /** 秒 */
  active: number
}

export interface Count {
  name: string
  count: number
}

export interface AppRow {
  id: string
  name: string
  active: number
  keys: number
  clicks: number
}

export interface AppRef {
  id: string
  name: string
}

export interface Summary {
  from: string
  to: string
  totals: Totals
  keys: Count[]
  combos: Count[]
  buttons: Count[]
  hours: Totals[]
  days: (Totals & { day: string })[]
  apps: AppRow[]
  firstDay: string | null
}

export interface DisplayMap {
  id: string
  x: number
  y: number
  width: number
  height: number
  cols: number
  rows: number
  /** [列, 行, 次数] */
  cells: [number, number, number][]
  max: number
  total: number
}

export interface Status {
  os: string
  unsupported: string | null
  permission: 'granted' | 'denied' | 'not_required'
  enabled: boolean
  capturing: boolean
  error: string | null
  app: AppRef | null
  today: Totals
  excluded: AppRef[]
}

export type Range = 'today' | 'yesterday' | 'week' | 'month' | 'all'

export type Platform = 'mac' | 'pc'
