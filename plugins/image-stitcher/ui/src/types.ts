import type { AppError } from '@toolforge/plugin-ui-sdk'

export type Layout = 'horizontal' | 'vertical' | 'grid'
export type Align = 'start' | 'center' | 'end'
export type BackgroundKind = 'transparent' | 'white' | 'black' | 'color'
export type Format = 'png' | 'jpeg' | 'webp'

export const FORMATS: Format[] = ['png', 'jpeg', 'webp']

export const FORMAT_LABELS: Record<Format, string> = {
  png: 'PNG',
  jpeg: 'JPEG',
  webp: 'WebP',
}

export interface InspectEntry {
  path: string
  name: string
  width: number
  height: number
  thumbnail: string | null
  error: AppError | null
}

export interface Report {
  output: string
  width: number
  height: number
  bytes: number
  elapsedMs: number
  thumbnail: string
}
