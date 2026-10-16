export type Format = 'json' | 'arb' | 'yaml' | 'properties'
export type Issue = 'missing' | 'extra' | 'empty' | 'untranslated' | 'placeholders' | 'duplicate'
export type Fill = 'none' | 'empty' | 'base'

export interface LoadedFile {
  path: string
  name: string
  text: string
}

export interface Cell {
  value: string | null
  issues: Issue[]
  missingPlaceholders?: string[]
  extraPlaceholders?: string[]
}

export interface Row {
  key: string
  cells: Cell[]
}

export interface Summary {
  name: string
  format: Format | null
  keys: number
  missing: number
  extra: number
  empty: number
  untranslated: number
  placeholders: number
  duplicate: number
  coverage: number
  writable: boolean
}

export interface Comparison {
  files: Summary[]
  rows: Row[]
}

export interface WriteResult {
  text: string
  added: string[]
  updated: string[]
}

export const ISSUES: Issue[] = ['missing', 'extra', 'empty', 'untranslated', 'placeholders', 'duplicate']
export const EXTENSIONS = ['json', 'arb', 'yaml', 'yml', 'properties']
export const baseName = (path: string) => path.split(/[\\/]/).pop() ?? path
