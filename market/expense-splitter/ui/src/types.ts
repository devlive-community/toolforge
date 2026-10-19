export interface Member {
  id: string
  name: string
}

export interface Weight {
  id: string
  shares: number
}

/** cents 字段是「元」的十进制字符串（如 "12.34"），由后端解析为分，避免前端浮点 */
export interface Amount {
  id: string
  cents: string
}

export type Split =
  | { mode: 'equal'; participants: string[] }
  | { mode: 'shares'; weights: Weight[] }
  | { mode: 'exact'; amounts: Amount[] }

export interface Expense {
  id: string
  title: string
  date?: string | null
  /** 元的十进制字符串 */
  total: string
  payer: string
  split: Split
}

export interface Settlement {
  id: string
  from: string
  to: string
  /** 元的十进制字符串 */
  cents: string
  date?: string | null
}

export interface Activity {
  name: string
  members: Member[]
  expenses: Expense[]
  settlements: Settlement[]
}

export interface Allocation {
  id: string
  cents: number
}

export interface ExpenseSplit {
  id: string
  title: string
  date?: string | null
  payer: string
  totalCents: number
  allocations: Allocation[]
}

export interface Balance {
  id: string
  name: string
  paid: number
  owed: number
  settledOut: number
  settledIn: number
  net: number
}

export interface Transfer {
  from: string
  to: string
  cents: number
}

export interface Report {
  expenses: ExpenseSplit[]
  balances: Balance[]
  transfers: Transfer[]
  totalCents: number
}

export interface Exported {
  content: string
  ext: string
}

export interface Diag {
  code: string
  params?: Record<string, unknown>
}
