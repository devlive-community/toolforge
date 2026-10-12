import type { AppError } from '@toolforge/plugin-ui-sdk'

export type Format = 'png' | 'jpeg' | 'webp'

export const FORMATS: Format[] = ['png', 'jpeg', 'webp']

export const FORMAT_LABELS: Record<Format, string> = {
  png: 'PNG',
  jpeg: 'JPEG',
  webp: 'WebP',
}

export type ResizeMode = 'none' | 'scale' | 'fit'

export type Resize =
  | { mode: 'none' }
  | { mode: 'scale'; percent: number }
  | { mode: 'fit'; maxWidth: number; maxHeight: number }

export interface FileResult {
  path: string
  name: string
  output: string | null
  format: Format | null
  beforeBytes: number
  afterBytes: number
  width: number
  height: number
  newWidth: number
  newHeight: number
  thumbnail: string | null
  error: AppError | null
}

export interface Report {
  files: FileResult[]
  succeeded: number
  elapsedMs: number
}
