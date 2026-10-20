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

export type Paper = 'a4' | 'letter'
export type Orientation = 'portrait' | 'landscape'

export interface SheetSettings {
  paper: Paper
  orientation: Orientation
  margin: number
  columns: number
  rows: number
  gap: number
  labelHeight: number
  fontSize: number
  title: string
  showNumber: boolean
  showName: boolean
  showPageNumber: boolean
}

export const DEFAULT_SHEET_SETTINGS: SheetSettings = {
  paper: 'a4',
  orientation: 'portrait',
  margin: 10,
  columns: 4,
  rows: 5,
  gap: 4,
  labelHeight: 8,
  fontSize: 7,
  title: '',
  showNumber: true,
  showName: true,
  showPageNumber: true,
}

export interface SheetCell {
  number: number
  x: number
  y: number
  w: number
  h: number
  imageX: number
  imageY: number
  imageW: number
  imageH: number
  lines: string[]
}

export interface SheetPage {
  number: number
  cells: SheetCell[]
}

export interface SheetPlan {
  pageSize: [number, number]
  labelFontSize: number
  title: string | null
  titleFontSize: number | null
  pageFontSize: number | null
  pages: SheetPage[]
}

export interface SheetReport {
  output: string
  pages: number
  images: number
  bytes: number
  elapsedMs: number
  missingGlyphs: string[]
}
