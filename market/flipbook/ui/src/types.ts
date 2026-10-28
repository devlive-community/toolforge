/** 与后端 project.rs / lib.rs 的结构一致（camelCase） */
export interface Project {
  id: string
  name: string
  width: number
  height: number
  delayMs: number
  /** GIF 导出底色 */
  background: string
  /** 当前帧顺序（帧编号） */
  frames: string[]
  /** 存储中可能存在的帧编号（含撤销历史引用的） */
  pool: string[]
  nextId: number
}

export type Op =
  | { kind: 'add'; index: number }
  | { kind: 'duplicate'; index: number }
  | { kind: 'delete'; index: number }
  | { kind: 'move'; from: number; to: number }
  | { kind: 'replace'; index: number; size: number }
  | { kind: 'delay'; ms: number }
  | { kind: 'background'; color: string }
  | { kind: 'rename'; name: string }
  | { kind: 'restore'; frames: string[] }

export interface Edit {
  project: Project
  created?: { id: string; source?: string }
}

export interface Limits {
  sizes: { width: number; height: number }[]
  maxFrames: number
  maxProjectMb: number
  minDelay: number
  maxDelay: number
  delayStep: number
  minBrush: number
  maxBrush: number
  palette: string[]
  backgrounds: string[]
}

/** 作品列表中的一项（只放摘要，帧内容各自单独存键） */
export interface WorkMeta {
  id: string
  name: string
  width: number
  height: number
  frames: number
  updatedAt: string
}

export type Tool = 'brush' | 'eraser'

export interface FrameOut {
  frame: string
  size: number
}

export interface ExportOut {
  format: string
  frames: number
  files: number
  bytes: number
}
