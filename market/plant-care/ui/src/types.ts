export interface Plan {
  id: string
  name: string
  first: string
  interval: number
  paused: boolean
  postponed: { base: string; until: string } | null
}
export interface Plant {
  id: string
  name: string
  variety: string
  location: string
  note: string
  archived: boolean
  cover: string | null
  plans: Plan[]
}
export interface PlantSummary {
  id: string
  name: string
  variety: string
  location: string
  archived: boolean
  thumbnail: string | null
}
export interface Care {
  id: string
  plantId: string
  plantName: string
  name: string
  first: string
  interval: number
  paused: boolean
  archived: boolean
  baseDue: string
  due: string
  status: string
  days: number
  last: string | null
  postponed: boolean
}
export interface Entry {
  id: string
  kind: 'care' | 'photo'
  date: string
  note: string
  name?: string
  voided?: boolean
  thumbnail?: string
  width?: number
  height?: number
  bytes?: number
}
export interface Photo {
  id: string
  plantId: string
  date: string
  note: string
  data: string
  thumbnail: string
  width: number
  height: number
  bytes: number
}
export interface Page<T> {
  items: T[]
  offset: number
  limit: number
  total: number
}
export interface Garden {
  schemaVersion: number
  revision: number
  selectedId: string | null
  plants: unknown[]
  records: unknown[]
  photos: unknown[]
}
export type Section = 'today' | 'plans' | 'timeline' | 'gallery'
export interface Query {
  section: Section
  offset: number
  plantsOffset: number
  filter: string
  search: string
}
export interface View {
  today: string
  selected: Plant | null
  plants: Page<PlantSummary>
  page: Page<Care | Entry>
  counts: { plants: number; overdue: number; today: number; photos: number; records: number; storedBytes: number; storageLimit: number }
}
export interface Result {
  requestId: string
  baseRevision: number
  state: Garden | null
  view: View
  preview: boolean
  previewDue: string | null
}
export interface Op {
  type: string
  [key: string]: unknown
}
export interface Editor {
  kind: 'plant' | 'editPlant' | 'plan' | 'editPlan' | 'complete' | 'postpone' | 'photo'
  care?: Care
  plantId?: string
  path?: string
}
export function emptyGarden(): Garden {
  return { schemaVersion: 1, revision: 0, selectedId: null, plants: [], records: [], photos: [] }
}
export const emptyQuery: Query = { section: 'today', offset: 0, plantsOffset: 0, filter: 'active', search: '' }
