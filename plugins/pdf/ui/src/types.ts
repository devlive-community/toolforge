export interface Metadata {
  title: string
  author: string
  subject: string
  keywords: string
  creator: string
  producer: string
}

export interface PageInfo {
  number: number
  width: number
  height: number
  rotate: number
}

export interface Info {
  path: string
  name: string
  size: number
  version: string
  encrypted: boolean
  pages: PageInfo[]
  metadata: Metadata
}

export interface PdfFile {
  id: number
  info: Info
  password: string | null
  /** 文件标识色序号 */
  color: number
}

export interface PageItem {
  key: string
  file: number
  page: number
  /** 额外旋转 */
  rotate: number
}

export const FILE_COLORS = ['bg-primary', 'bg-info', 'bg-warning', 'bg-danger', 'bg-success', 'bg-fg-muted']

export function fileName(path: string) {
  return path.split(/[\\/]/).pop() ?? path
}
