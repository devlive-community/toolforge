import type { AppError } from '@toolforge/plugin-ui-sdk'

export type Target = 'keep' | 'png' | 'jpeg' | 'webp' | 'gif' | 'bmp' | 'ico' | 'tiff'

export const TARGETS: Target[] = ['keep', 'jpeg', 'png', 'webp', 'gif', 'bmp', 'ico', 'tiff']

export const TARGET_LABELS: Record<Exclude<Target, 'keep'>, string> = {
  png: 'PNG',
  jpeg: 'JPEG',
  webp: 'WebP',
  gif: 'GIF',
  bmp: 'BMP',
  ico: 'ICO',
  tiff: 'TIFF',
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
  format: Target | null
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
  totalBefore: number
  totalAfter: number
  elapsedMs: number
}
