import type { AppError } from '@toolforge/plugin-ui-sdk'

export type Confidence = 'certain' | 'likely' | 'unsure'
export type LineEndings = 'none' | 'lf' | 'crlf' | 'cr' | 'mixed'
export type NewLine = 'keep' | 'lf' | 'crlf'
export type Output = 'inplace' | 'folder'

export interface FileInfo {
  path: string
  name: string
  size: number
  encoding: string | null
  bom: boolean
  ascii: boolean
  confidence: Confidence | null
  lineEndings: LineEndings | null
  error: AppError | null
}

export interface Scanned {
  files: FileInfo[]
  skipped: { binary: number; tooMany: number }
}

export interface Preview {
  encoding: string
  text: string
  truncated: boolean
  malformed: boolean
  lineEndings: LineEndings
  unmappable: { char: string; code: string; line: number } | null
}

export interface Item {
  path: string
  status: 'converted' | 'unchanged' | 'failed'
  output: string | null
  error: AppError | null
}

export interface Converted {
  items: Item[]
  summary: { converted: number; unchanged: number; failed: number }
}

/** 支持 BOM 的编码 */
export const UNICODE = ['UTF-8', 'UTF-16LE', 'UTF-16BE']
