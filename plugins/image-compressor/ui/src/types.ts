export type Format = 'keep' | 'jpeg' | 'png' | 'webp'
export type PngMode = 'lossy' | 'lossless'

export interface Options {
  format: Format
  quality: number
  pngMode: PngMode
  colors: number
  dither: boolean
  keepMetadata: boolean
  maxSize: number
}

export const DEFAULT_OPTIONS: Options = {
  format: 'keep',
  quality: 75,
  pngMode: 'lossy',
  colors: 256,
  dither: true,
  keepMetadata: false,
  maxSize: 0,
}

export interface Output {
  dir: string | null
  suffix: string
}

export const DEFAULT_OUTPUT: Output = { dir: null, suffix: '-min' }

export interface Entry {
  path: string
  name: string
  size: number
}

export interface AppError {
  code: string
  params?: Record<string, unknown>
}

export interface FileResult {
  path: string
  name: string
  output: string | null
  format: Exclude<Format, 'keep'> | null
  beforeBytes: number
  afterBytes: number
  width: number
  height: number
  keptOriginal: boolean
  ms: number
  error: AppError | null
}

export interface Report {
  files: FileResult[]
  succeeded: number
  totalBefore: number
  totalAfter: number
  elapsedMs: number
}

export const FORMAT_LABELS: Record<Exclude<Format, 'keep'>, string> = { jpeg: 'JPEG', png: 'PNG', webp: 'WebP' }
export const MAX_SIZES = [0, 1280, 1920, 2560, 3840]
export const COLORS = [256, 128, 64, 32, 16]
