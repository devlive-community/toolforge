export type Mode = 'json' | 'json5'
export type Indent = '2' | '4' | 'tab'
export type Tab = 'format' | 'minify' | 'validate' | 'escape' | 'tree' | 'diff'

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
