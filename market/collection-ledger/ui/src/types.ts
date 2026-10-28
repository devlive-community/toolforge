/** 与后端 model.rs / ops.rs / view.rs / backup.rs 的结构一致（camelCase） */
export interface Item {
  id: string
  name: string
  series: string
  edition: string
  code: string
  category: string
  location: string
  condition: string
  notes: string
  owned: number
  archived: boolean
  hasPhoto: boolean
  created: string
}

export interface Return {
  id: string
  qty: number
  date: string
  note: string
}

export interface Loan {
  id: string
  itemId: string
  borrower: string
  qty: number
  date: string
  due: string | null
  note: string
  returns: Return[]
}

/** 台账整体由界面原样保存、原样回传，结构细节由 Rust 校验 */
export interface Ledger {
  schema: number
  revision: number
  items: Item[]
  loans: Loan[]
  recentOps: string[]
  undo: { kind: string } | null
}

export type Fields = Pick<Item, 'name' | 'series' | 'edition' | 'code' | 'category' | 'location' | 'condition' | 'notes'>

export type Op =
  | { kind: 'create'; fields: Fields; owned: number }
  | { kind: 'edit'; itemId: string; fields: Fields }
  | { kind: 'setOwned'; itemId: string; owned: number }
  | { kind: 'lend'; itemId: string; borrower: string; qty: number; date: string; due: string | null; note: string }
  | { kind: 'return'; loanId: string; qty: number; date: string; note: string }
  | { kind: 'archive'; itemId: string; archived: boolean }
  | { kind: 'delete'; itemId: string }
  | { kind: 'setPhoto'; itemId: string; hasPhoto: boolean }
  | { kind: 'undo' }

export interface Impact {
  kind: string
  itemId: string | null
  itemName: string
  owned: [number, number]
  available: [number, number]
  outstanding: [number, number]
  loanOutstanding: [number, number] | null
  borrower: string | null
}

export interface Outcome {
  ledger: Ledger | null
  impact: Impact
  duplicate: boolean
  createdId: string | null
}

export type ItemView = Item & { available: number; outstanding: number; activeLoans: number; hasLoans: boolean }
export type DueState = 'overdue' | 'soon' | 'ok' | 'none'
export type LoanView = Loan & { itemName: string; returned: number; outstanding: number; dueState: DueState; daysLeft: number | null }
export type ItemState = 'all' | 'inHand' | 'lent' | 'archived'

export interface View {
  revision: number
  items: ItemView[]
  total: number
  pending: LoanView[]
  pendingTotal: number
  summary: { items: number; archived: number; owned: number; available: number; outstanding: number; activeLoans: number; overdue: number }
  locations: string[]
  categories: string[]
  undo: string | null
}

export interface RestoreReport {
  itemsAdded: number
  loansAdded: number
  itemsSkipped: number
  loansSkipped: number
  photos: number
  itemsRemoved: number
  loansRemoved: number
}

export interface Restored {
  ledger: Ledger | null
  photos: Record<string, string>
  removePhotos: string[]
  report: RestoreReport
}
