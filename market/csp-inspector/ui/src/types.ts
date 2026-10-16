export interface Diag {
  code: string
  params?: Record<string, unknown>
}

export interface ResourceInput {
  type: string
  url?: string | null
  nonce?: string | null
  hash?: string | null
}

export interface DirectiveView {
  name: string
  values: string[]
}

export interface PolicyView {
  index: number
  reportOnly: boolean
  raw: string
  directives: DirectiveView[]
  diagnostics: Diag[]
}

export interface PolicyVerdict {
  index: number
  reportOnly: boolean
  status: 'allowed' | 'blocked' | 'undetermined' | 'unrestricted'
  directive: string | null
  matched: string | null
}

export interface ResourceVerdict {
  index: number
  kind: string
  url: string | null
  status: 'allowed' | 'blocked' | 'undetermined' | 'unrestricted'
  directive: string | null
  matched: string | null
  policies: PolicyVerdict[]
  reasons: Diag[]
}

export interface Stats {
  allowed: number
  blocked: number
  undetermined: number
  unrestricted: number
}

export interface Report {
  document: string | null
  policies: PolicyView[]
  resources: ResourceVerdict[]
  diagnostics: Diag[]
  stats: Stats
}

export interface Exported {
  content: string
  ext: string
}
