export interface Diag {
  code: string
  params?: Record<string, unknown>
}

export interface SourceInfo {
  name: string | null
  hasContent: boolean
  segments: number
}

export interface SectionInfo {
  index: number
  offsetLine: number
  offsetColumn: number
  hasMap: boolean
  url: string | null
  sources: number
  segments: number
}

export interface Overview {
  handle: number
  kind: 'regular' | 'index'
  file: string | null
  sourceRoot: string | null
  sources: SourceInfo[]
  names: number
  segments: number
  mappedLines: number
  sections: SectionInfo[] | null
  diagnostics: Diag[]
}

export interface ContextLine {
  line: number
  text: string
}

export interface Context {
  lines: ContextLine[]
  target: number
}

export interface SectionPoint {
  line: number
  column: number
}

export interface Resolved {
  source: string | null
  sourceIndex: number
  line: number
  column: number
  name: string | null
  section: SectionPoint | null
  context: Context | null
}

export type FrameStatus = 'resolved' | 'no_mapping' | 'unrelated' | 'unassociated' | 'no_column' | 'message'

export interface FrameOut {
  index: number
  raw: string
  kind: 'frame' | 'message'
  function: string | null
  url: string | null
  line: number | null
  column: number | null
  status: FrameStatus
  reason: Diag | null
  resolved: Resolved | null
}

export interface Stats {
  resolved: number
  no_mapping: number
  unrelated: number
  unassociated: number
  no_column: number
  messages: number
}

export interface Symbolication {
  asset: string | null
  frames: FrameOut[]
  diagnostics: Diag[]
  stats: Stats
}

export interface Exported {
  content: string
  ext: string
}
