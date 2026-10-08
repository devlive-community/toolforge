export type Status = 'same' | 'changed' | 'leftOnly' | 'rightOnly' | 'mismatch'
export type Method = 'quick' | 'content'
export type Filter = 'all' | 'diff' | 'changed' | 'leftOnly' | 'rightOnly'

export interface Info {
  dir: boolean
  size: number
  modified: number
}

export interface Item {
  path: string
  name: string
  depth: number
  dir: boolean
  status: Status
  left: Info | null
  right: Info | null
  newer: 'left' | 'right' | null
}

export interface Summary {
  same: number
  changed: number
  leftOnly: number
  rightOnly: number
  mismatch: number
}

export interface Compared {
  left: string
  right: string
  items: Item[]
  summary: Summary
}

export interface DiffLine {
  number: number
  text: string
}

export interface DiffRow {
  tag: 'equal' | 'delete' | 'insert' | 'change'
  left: DiffLine | null
  right: DiffLine | null
}

export interface Diff {
  rows: DiffRow[]
  added: number
  removed: number
  truncated: boolean
}

/** 状态对应的文字颜色（Tailwind 需要完整类名） */
export const STATUS_TEXT: Record<Status, string> = {
  same: 'text-fg-subtle',
  changed: 'text-warning',
  leftOnly: 'text-info',
  rightOnly: 'text-success',
  mismatch: 'text-danger',
}

export const STATUS_BADGE: Record<Status, 'neutral' | 'warning' | 'info' | 'success' | 'danger'> = {
  same: 'neutral',
  changed: 'warning',
  leftOnly: 'info',
  rightOnly: 'success',
  mismatch: 'danger',
}
