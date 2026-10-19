// 数织数据模型：进度与偏好由 usePluginState 持久化（ToolForge SQLite）

/** 单格状态：0=未定 1=黑 2=标空 */
export const CELL_UNKNOWN = 0
export const CELL_BLACK = 1
export const CELL_WHITE = 2

export interface Clues {
  size: number
  rows: number[][]
  cols: number[][]
}

export interface Puzzle {
  size: number
  clues: Clues
  solution: boolean[]
  attempts: number
}

export interface CheckOut {
  done: boolean
  rowDone: boolean[]
  colDone: boolean[]
  rowConflict: boolean[]
  colConflict: boolean[]
}

export interface HintOut {
  found: boolean
  row?: number
  col?: number
  black?: boolean
  via?: number
}

export interface PaperOut {
  html: string
}

export interface NonogramState {
  size: number
  clues: Clues | null
  solution: boolean[] | null
  /** 玩家棋盘 */
  board: number[] | null
  /** 操作历史（快照栈），一组拖动 = 一步 */
  history: number[][]
  redo: number[][]
  usedHint: boolean
  usedSolution: boolean
  /** 检查后的行列完成标记（仅主动检查时更新） */
  rowDone: boolean[] | null
  colDone: boolean[] | null
  rowConflict: boolean[] | null
  colConflict: boolean[] | null
  won: boolean
}

export const initialNonogramState = (): NonogramState => ({
  size: 5,
  clues: null,
  solution: null,
  board: null,
  history: [],
  redo: [],
  usedHint: false,
  usedSolution: false,
  rowDone: null,
  colDone: null,
  rowConflict: null,
  colConflict: null,
  won: false,
})
