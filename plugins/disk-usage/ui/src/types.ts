export type Category = 'images' | 'videos' | 'audio' | 'documents' | 'archives' | 'code' | 'other'

export interface Summary {
  id: number
  name: string
  size: number
  files: number
  dir: boolean
  category: Category
  items: number
  path: string
}

export interface Listing {
  node: Summary
  path: string
  crumbs: { id: number; name: string }[]
  children: Summary[]
  others: [number, number] | null
}

export interface Tile {
  id: number | null
  name: string
  size: number
  dir: boolean
  category: Category
  x: number
  y: number
  w: number
  h: number
  depth: number
  merged: number
}

export interface TypeStat {
  category: Category
  size: number
  files: number
}

export interface LargeFile {
  id: number
  name: string
  path: string
  size: number
  category: Category
}

export interface ScanResult {
  id: number
  root: Summary
  denied: number
  elapsedMs: number
}

export interface Settings {
  root: string
  includeHidden: boolean
}

/** 类型颜色：复用工具图标的渐变色 */
export const CATEGORY_CLASS: Record<Category, string> = {
  images: 'tile-green',
  videos: 'tile-pink',
  audio: 'tile-cyan',
  documents: 'tile-blue',
  archives: 'tile-orange',
  code: 'tile-violet',
  other: 'bg-border-strong',
}
