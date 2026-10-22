export interface Player {
  seat: number
  name: string
}
export interface Ranking extends Player {
  total: number
  rank: number
  leading: boolean
}
export interface Round {
  number: number
  scores: number[]
  cumulative: number[]
}
export interface CellChange {
  round: number
  seat: number
  before: number | null
  after: number | null
  draft: boolean
}
export interface Event {
  sequence: number
  kind: string
  changes: CellChange[]
  beforeText: string | null
  afterText: string | null
}
export interface Page<T> {
  items: T[]
  offset: number
  limit: number
  total: number
}
export interface Summary {
  id: string
  name: string
  players: number
  rounds: number
  finished: boolean
}
export interface Game {
  id: string
  name: string
  players: Player[]
  direction: 'high' | 'low'
  roundLimit: number | null
  steps: number[]
  draft: (number | null)[]
  finished: boolean
  confirmed: number
  nextRound: number
  canUndo: boolean
  canAdd: boolean
  canFinish: boolean
  ranking: Ranking[]
  rounds: Page<Round>
  events: Page<Event>
}
export interface View {
  gameOptions: Summary[]
  games: Page<Summary>
  active: Game | null
}
export interface State {
  schemaVersion: number
  revision: number
  selectedId: string | null
  games: unknown[]
}
export interface Result {
  requestId: string
  baseRevision: number
  state: State | null
  view: View
}
export interface Exported {
  content?: string
  saved?: boolean
}
export type Op = { type: string; [key: string]: unknown }
export type Editor = { kind: 'new' | 'copy' | 'rename' | 'edit'; round?: Round }
export function emptyState(): State {
  return { schemaVersion: 1, revision: 0, selectedId: null, games: [] }
}
