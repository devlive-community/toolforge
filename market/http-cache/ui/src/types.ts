export type Role = 'private' | 'shared'
export interface Scenario {
  name: string
  role: Role
  method: string
  status: string
  requestHeaders: string
  responseHeaders: string
  newRequestHeaders: string | null
  requestTime: string
  responseTime: string
  now: string
}
export interface Note {
  code: string
  params: Record<string, string | number>
}
export interface Condition {
  key: string
  status: string
  note: Note
}
export interface Directive {
  context: string
  name: string
  value: string | null
  valid: boolean
  supported: boolean
}
export interface VaryRow {
  name: string
  stored: string | null
  presented: string | null
  status: string
}
export interface Header {
  name: string
  value: string
}
export interface Timing {
  date: number | null
  requestTime: number | null
  responseTime: number | null
  now: number | null
  ageValue: number
  apparentAge: number | null
  responseDelay: number | null
  correctedAgeValue: number | null
  correctedInitialAge: number | null
  residentTime: number | null
  currentAge: number | null
  lifetime: number | null
  lifetimeSource: string
  remaining: number | null
  freshness: string
}
export interface Summary {
  name: string
  role: Role
  storage: string
  reuse: string
  freshness: string
  lifetime: number | null
  currentAge: number | null
  remaining: number | null
  lifetimeSource: string
  diagnostics: number
}
export interface Inspected {
  handle: string
  summaries: Summary[]
}
export type Section = 'conditions' | 'timing' | 'vary' | 'directives' | 'diagnostics' | 'headers'
export interface View {
  handle: string
  case: number
  section: string
  summary: Summary
  timing: Timing
  conditional: string
  sameRequestAssumed: boolean
  onlyIfCachedUnsatisfied: boolean
  headers: { request: Header[]; response: Header[]; presented: Header[] }
  page: { items: (Condition | Note | Directive | VaryRow)[]; offset: number; limit: number; total: number }
}
export interface Exported {
  content?: string
  saved?: boolean
}
