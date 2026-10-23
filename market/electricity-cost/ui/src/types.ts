export interface Usage {
  power: string
  unit: 'w' | 'kw'
  peak: string
  shoulder: string
  valley: string
}
export interface Appliance {
  id: string
  name: string
  quantity: number
  usage: Usage
  alternative: Usage | null
}
export interface Tier {
  upTo: string | null
  rate: string
}
export type Pricing = { kind: 'flat'; rate: string } | { kind: 'tiered'; tiers: Tier[] } | { kind: 'tou'; peak: string; shoulder: string; valley: string }
export interface Tariff {
  id: string
  name: string
  pricing: Pricing
}
export interface Input {
  days: number
  currency: string
  appliances: Appliance[]
  tariffs: Tariff[]
}
export interface Band {
  index: number
  upTo: string | null
  rate: string
  kwh: string
  cost: string
  reached: boolean
}
export interface Totals {
  dailyKwh: string
  monthlyKwh: string
  monthlyCost: string
  dailyCost: string
  annualCost: string
  highestTier: number | null
  remainingKwh: string | null
  bands: Band[]
}
export interface Summary {
  tariff: Tariff
  baseline: Totals
  alternative: Totals
  annualSaving: string
  savingDirection: 'saving' | 'increase' | 'same'
}
export interface Report {
  handle: string
  currency: string
  days: number
  applianceCount: number
  summaries: Summary[]
}
export interface Row {
  id: string
  name: string
  baselineKwh: string
  alternativeKwh: string
  baselineCost: string
  alternativeCost: string
  sharePercent: string
}
export interface Page<T> {
  items: T[]
  offset: number
  limit: number
  total: number
}
export interface Book {
  schemaVersion: number
  revision: number
  tariffs: Tariff[]
}
export interface BookResult {
  state: Book
  page: Page<Tariff>
}
