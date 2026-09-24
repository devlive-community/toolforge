export interface Heading {
  level: number
  text: string
  id: string
  line: number
}

export interface Stats {
  chars: number
  words: number
  lines: number
  readingMinutes: number
  links: number
  images: number
  codeBlocks: number
  tasks: number
  tasksDone: number
}

export interface Rendered {
  html: string
  outline: Heading[]
  stats: Stats
  title: string | null
}

export interface Draft {
  text: string
  /** 关联的磁盘文件；为空表示尚未保存过 */
  path: string | null
  /** 有未保存到磁盘的修改 */
  dirty: boolean
}

export type ViewMode = 'edit' | 'split' | 'preview'
