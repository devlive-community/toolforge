export type Mode = 'json' | 'json5'
export type Indent = '2' | '4' | 'tab'
export type Tab = 'format' | 'minify' | 'validate' | 'canonical' | 'escape' | 'tree' | 'diff'

export interface Stats {
  objects: number
  arrays: number
  strings: number
  numbers: number
  booleans: number
  nulls: number
  keys: number
  maxDepth: number
  lines: number
  chars: number
  bytes: number
}

/** 与 Rust `Output` 对应 */
export interface ProcessResult {
  output: string
  docId: number | null
  stats: Stats | null
  /** 仅 JCS：精度发生变化的数字个数 */
  lossyNumbers?: number
  elapsedMs: number
}

export type Segment = string | number
export type NodeKind = 'object' | 'array' | 'string' | 'number' | 'boolean' | 'null'

export interface TreeNode {
  key: Segment
  kind: NodeKind
  size: number
  preview: string
}

export interface TreePage {
  kind: NodeKind
  nodes: TreeNode[]
  total: number
}

export interface DiffItem {
  path: string
  change: 'added' | 'removed' | 'changed'
  left: string | null
  right: string | null
}

export interface DiffReport {
  items: DiffItem[]
  added: number
  removed: number
  changed: number
  truncated: boolean
  elapsedMs: number
}

export interface PatchResult {
  output: string
  operations: number | null
  nullPaths: string[]
}
