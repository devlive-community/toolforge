/** 冰箱食材批次台账数据模型（与后端 serde 模型对应） */

export interface Batch {
  id: string
  name: string
  /** 位置 id */
  location: string
  unit: string
  /** 入库量（原始单位）；有历史后修改以 adjust 变动留痕 */
  intake: number
  purchasedDate: string
  /** 用户抄录的标签日期；缺省 = 未知 */
  labelDate?: string
  /** 开封日期：只记录不扣量 */
  openedDate?: string
  /** 用户自设的开封后关注日 */
  attentionDate?: string
}

export type MovementKind = 'adjust' | 'consume' | 'discard'

export interface Movement {
  id: string
  batchId: string
  kind: MovementKind
  /** 批次单位数量；adjust 带符号 */
  amount: number
  date: string
}

export interface Location {
  id: string
  name: string
}

export interface Settings {
  soonDays: number
  locations: Location[]
}

export type UnitId =
  | 'g'
  | 'kg'
  | 'ml'
  | 'l'
  | 'piece'
  | 'pack'
  | 'bag'
  | 'box'
  | 'bottle'
  | 'can'

export const UNITS: UnitId[] = ['g', 'kg', 'ml', 'l', 'piece', 'pack', 'bag', 'box', 'bottle', 'can']

const UNIT_DIM: Record<UnitId, 'weight' | 'volume' | 'count'> = {
  g: 'weight',
  kg: 'weight',
  ml: 'volume',
  l: 'volume',
  piece: 'count',
  pack: 'count',
  bag: 'count',
  box: 'count',
  bottle: 'count',
  can: 'count',
}

/** 同维可选单位：计数单位只允许与批次单位完全一致 */
export function compatibleUnits(batchUnit: string): UnitId[] {
  const dim = UNIT_DIM[batchUnit as UnitId]
  if (!dim) return []
  if (dim === 'count') return [batchUnit as UnitId]
  return UNITS.filter((unit) => UNIT_DIM[unit] === dim)
}

export type DateState = 'overdue' | 'soon' | 'ok' | 'unknown'

export interface BatchView extends Batch {
  remaining: number
  remainingBase: number
  consumed: number
  discarded: number
  adjusted: number
  status: 'active' | 'depleted'
  opened: boolean
  labelState: DateState
  attentionState: 'none' | DateState
  lastMovementId?: string
  movementCount: number
}

export interface ReviewOut {
  batches: BatchView[]
}

export interface ApplyOut {
  movement?: Movement
  batch?: Batch
  remainingBase: number
  remaining: number
}

export const DEFAULT_LOCATIONS: Location[] = [
  { id: 'fridge', name: 'fridge' },
  { id: 'freezer', name: 'freezer' },
  { id: 'pantry', name: 'pantry' },
]

export const DEFAULT_SETTINGS: Settings = { soonDays: 3, locations: DEFAULT_LOCATIONS }

export const MAX_BATCHES = 200
export const MAX_SOON_DAYS = 90
/** 数值显示：三位小数内去尾零 */
export function formatAmount(value: number): string {
  return String(parseFloat(value.toFixed(3)))
}

export function formatDate(date: Date): string {
  const y = date.getFullYear()
  const m = String(date.getMonth() + 1).padStart(2, '0')
  const d = String(date.getDate()).padStart(2, '0')
  return `${y}-${m}-${d}`
}

export function todayText(): string {
  return formatDate(new Date())
}
