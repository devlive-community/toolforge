import type { AppError } from '@toolforge/plugin-ui-sdk'

export interface Catalog {
  types: string[]
  public: { id: string; address: string }[]
}

export interface Answer {
  name: string
  recordType: string
  ttl: number
  value: string
  fakeIp: boolean
}

export interface ServerResult {
  server: string
  address: string | null
  ms: number
  answers: Answer[]
  error: AppError | null
}

export interface Report {
  query: string
  recordType: string
  results: ServerResult[]
}
