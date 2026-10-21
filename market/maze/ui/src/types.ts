export type Side = 'top' | 'right' | 'bottom' | 'left'
export interface Port {
  side: Side
  index: number
}
export interface Spec {
  algorithm: string
  rows: number
  cols: number
  entrance: Port
  exit: Port
  seed: string
}
export interface Draft {
  rows: string
  cols: string
  entranceSide: Side
  entranceIndex: string
  exitSide: Side
  exitIndex: string
  seed: string
}
export interface Score {
  steps: number
  hints: number
  usedAnswer: boolean
}
export interface Game {
  spec: Spec
  id: string
  history: number[]
  hints: number
  hintTarget: number | null
  revealed: boolean
  answerVisible: boolean
  showTrail: boolean
  completion: Score | null
}
export interface State {
  schemaVersion: number
  revision: number
  game: Game | null
}
export interface Point {
  x: number
  y: number
}
export interface Cell {
  id: number
  x: number
  y: number
  adjacent: boolean
}
export interface View {
  id: string
  code: string
  spec: Spec
  cells: Cell[]
  wallPath: string
  trailPath: string
  answerPath: string | null
  player: Point
  start: Point
  exit: Point
  hint: Point | null
  position: number
  steps: number
  visited: number
  routeLength: number
  cellCount: number
  hints: number
  revealed: boolean
  showTrail: boolean
  answerVisible: boolean
  finished: boolean
  completion: Score | null
}
export type Op =
  | { type: 'new'; draft: Draft; confirm: boolean }
  | { type: 'importCode'; code: string; confirm: boolean }
  | { type: 'move'; side: Side }
  | { type: 'moveTo'; cell: number }
  | { type: 'restart'; confirm: boolean }
  | { type: 'undo' | 'hint' | 'toggleAnswer' | 'toggleTrail' | 'markAnswer' }
export interface Result {
  requestId: string
  baseRevision: number
  state: State | null
  view: View | null
  notice: string
}
export interface Options {
  count: number
  perPage: number
  sheet: 'puzzle' | 'answer' | 'both'
}
export interface Preview {
  svg: string
  page: number
  total: number
  ids: string[]
  answer: boolean
}
export function emptyState(): State {
  return { schemaVersion: 1, revision: 0, game: null }
}
export function draftFor(spec?: Spec): Draft {
  return spec
    ? {
        rows: String(spec.rows),
        cols: String(spec.cols),
        entranceSide: spec.entrance.side,
        entranceIndex: String(spec.entrance.index),
        exitSide: spec.exit.side,
        exitIndex: String(spec.exit.index),
        seed: '',
      }
    : { rows: '12', cols: '12', entranceSide: 'left', entranceIndex: '1', exitSide: 'right', exitIndex: '12', seed: '' }
}
