export interface Problem {
  code: string
  message: string
  offset: number | null
}

export interface Section {
  id: number
  kind: string
  name: string | null
  offset: number
  size: number
  items: number | null
}

export interface Sizes {
  total: number
  header: number
  code: number
  data: number
  custom: number
  other: number
  withoutCustom: number
}

export interface Counts {
  types: number
  importedFunctions: number
  importedTables: number
  importedMemories: number
  importedGlobals: number
  importedTags: number
  functions: number
  tables: number
  memories: number
  globals: number
  tags: number
  exports: number
  elements: number
  data: number
  start: number | null
}

export interface Limits {
  index: number
  import: string | null
  min: number
  max: number | null
  pageSize: number | null
  element: string | null
  is64: boolean
  shared: boolean
}

export interface Producer {
  field: string
  values: string[]
}

export interface Validation {
  status: 'valid' | 'invalid' | 'needs_features'
  error: Problem | null
  required: string[]
  features: string[]
  detected: boolean
}

export interface FunctionRow {
  index: number
  name: string | null
  export: string | null
  signature: string
  size: number
  offset: number
  locals: number
}

export interface ImportRow {
  module: string
  name: string
  kind: string
  index: number
  desc: string
  offset: number
}

export interface ExportRow {
  name: string
  kind: string
  index: number
  desc: string
}

export interface Overview {
  token: number
  name: string
  version: number | null
  sections: Section[]
  sizes: Sizes
  counts: Counts
  memories: Limits[]
  tables: Limits[]
  producers: Producer[]
  parseError: Problem | null
  validation: Validation
  imports: number
  exports: number
  largest: FunctionRow[]
}

export interface Page<T> {
  total: number
  items: T[]
}

export interface WatInfo {
  bytes: number
  truncated: boolean
  functions: number
  outline: string
  outlineTruncated: boolean
}

export interface Exported {
  content: string
  ext: string
}

export type ListKind = 'imports' | 'exports' | 'functions'
