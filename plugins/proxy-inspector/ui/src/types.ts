export type SourceStatusKind = 'configured' | 'disabled' | 'notConfigured' | 'failed' | 'unsupported'

export interface SourceEntry {
  protocol: string
  host: string
  port: number
  warnings: string[]
}

export interface SourceStatus {
  kind: SourceStatusKind
  code?: string
}

export interface Source {
  id: string
  scope: string
  status: SourceStatus
  entries: SourceEntry[]
  autoDiscover: boolean | null
  pacUrl: string | null
  exclusions: string[]
  raw: Record<string, unknown> | null
}

export interface EnvVar {
  name: string
  value: string
  isExclusions: boolean
}

export interface Report {
  sources: Source[]
  envVars: EnvVar[]
  envAvailable: boolean
}

export const SOURCE_IDS = ['macos-sc', 'win-inet', 'win-http', 'gnome'] as const
