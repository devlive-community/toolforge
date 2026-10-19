export interface TemplateItem {
  name: string
  category: string
  unit: string
  base: number
  perPerson?: boolean
  perDay?: boolean
  note?: string | null
  weight?: number | null
}

export interface Bag {
  id: string
  name: string
  selfWeight?: number | null
}

export interface TripItem {
  id: string
  name: string
  category: string
  unit: string
  target: number
  packed: number
  owner?: string | null
  bagId?: string | null
  excluded: boolean
  weight?: number | null
  note?: string | null
}

export interface Trip {
  people: number
  days: number
  bags: Bag[]
  items: TripItem[]
}

export interface CategoryProgress {
  category: string
  target: number
  packed: number
}

export interface BagWeight {
  id: string
  name: string
  packedWeight: number
  selfWeight?: number | null
  totalWeight?: number | null
  unknownWeightItems: number
}

export interface UnpackedItem {
  id: string
  name: string
  category: string
  remaining: number
  owner?: string | null
  bagId?: string | null
}

export interface Summary {
  target: number
  packed: number
  excluded: number
  unknownWeightItems: number
  totalWeight: number
  categories: CategoryProgress[]
  bags: BagWeight[]
  unpacked: UnpackedItem[]
}

export interface Exported {
  content: string
  ext: string
  markdown?: string
}

export interface Diag {
  code: string
  params?: Record<string, unknown>
}
