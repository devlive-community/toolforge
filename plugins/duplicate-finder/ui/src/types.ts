export interface Copy {
  path: string
  modified: number
  root: number
  links: string[]
}

export interface Group {
  hash: string
  size: number
  files: Copy[]
  wasted: number
}

export interface Report {
  groups: Group[]
  scanned: number
  scannedBytes: number
  duplicates: number
  wasted: number
  elapsedMs: number
}

export interface Outcome {
  removed: string[]
  skipped: string[]
  freed: number
}

export type Strategy = 'newest' | 'oldest' | 'shortest' | 'firstFolder'
export type Kind = 'all' | 'images' | 'videos' | 'audio' | 'documents' | 'archives'

export const KIND_EXTENSIONS: Record<Kind, string[]> = {
  all: [],
  images: ['jpg', 'jpeg', 'png', 'gif', 'webp', 'heic', 'heif', 'bmp', 'tif', 'tiff', 'svg', 'raw', 'cr2', 'cr3', 'nef', 'arw', 'dng'],
  videos: ['mp4', 'mov', 'mkv', 'avi', 'wmv', 'm4v', 'webm', 'flv', '3gp'],
  audio: ['mp3', 'flac', 'wav', 'aac', 'm4a', 'ogg', 'wma', 'aiff'],
  documents: ['pdf', 'doc', 'docx', 'xls', 'xlsx', 'ppt', 'pptx', 'txt', 'md', 'rtf', 'csv', 'pages', 'numbers', 'key', 'epub'],
  archives: ['zip', 'rar', '7z', 'tar', 'gz', 'bz2', 'xz', 'dmg', 'iso'],
}

export const MIN_SIZES = [1, 1024, 100 * 1024, 1024 * 1024, 10 * 1024 * 1024]

export interface Settings {
  roots: string[]
  minSize: number
  includeHidden: boolean
  kind: Kind
}

export const DEFAULT_SETTINGS: Settings = { roots: [], minSize: 1, includeHidden: false, kind: 'all' }

/** 选择要保留的文件，返回其余文件的路径 */
export function autoSelect(group: Group, strategy: Strategy): string[] {
  const score = (file: Copy): [number, number, string] => {
    switch (strategy) {
      case 'newest':
        return [-file.modified, file.path.length, file.path]
      case 'oldest':
        return [file.modified, file.path.length, file.path]
      case 'shortest':
        return [file.path.length, 0, file.path]
      case 'firstFolder':
        return [file.root, file.path.length, file.path]
    }
  }
  const sorted = [...group.files].sort((a, b) => {
    const [x, y] = [score(a), score(b)]
    return x[0] - y[0] || x[1] - y[1] || x[2].localeCompare(y[2])
  })
  return sorted.slice(1).map((file) => file.path)
}
