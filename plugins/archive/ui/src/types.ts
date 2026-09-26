export type Kind = 'file' | 'dir' | 'link'
export type Output = 'zip' | 'sevenZ' | 'tarGz' | 'tarXz'
export type Conflict = 'rename' | 'overwrite' | 'skip'

export interface OpenInfo {
  id: number
  name: string
  path: string
  format: string
  size: number
  bytes: number
  files: number
  encrypted: boolean
  unsafePaths: number
  links: number
  elapsedMs: number
}

export interface Child {
  name: string
  path: string
  kind: Kind
  size: number
  packed: number | null
  modified: string | null
  files: number
  encrypted: boolean
  safe: boolean
}

export interface Preview {
  kind: 'text' | 'image' | 'binary' | 'tooLarge' | 'none'
  text: string | null
  dataUri: string | null
  truncated: boolean
  size: number
}

export interface Extracted {
  dest: string
  files: number
  dirs: number
  bytes: number
  skipped: number
  unsafePaths: number
  links: number
}

export interface Created {
  output: string
  files: number
  dirs: number
  bytes: number
  size: number
  skipped: number
}

export const FORMAT_LABELS: Record<string, string> = {
  zip: 'ZIP',
  sevenZ: '7z',
  tar: 'TAR',
  tarGz: 'tar.gz',
  tarBz2: 'tar.bz2',
  tarXz: 'tar.xz',
  tarZst: 'tar.zst',
  gz: 'gzip',
  bz2: 'bzip2',
  xz: 'xz',
  zst: 'zstd',
}

export const ARCHIVE_EXTENSIONS = ['zip', '7z', 'tar', 'gz', 'tgz', 'bz2', 'tbz2', 'xz', 'txz', 'zst', 'jar', 'apk', 'epub', 'docx', 'xlsx']
