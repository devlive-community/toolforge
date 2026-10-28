/** 与后端 model.rs / ops.rs / view.rs / backup.rs 的结构一致（camelCase） */
export type Kind = 'book' | 'film'
export type Status = 'want' | 'doing' | 'done' | 'paused'

export interface Work {
  id: string
  kind: Kind
  title: string
  creator: string
  year: number | null
  genre: string
  tags: string[]
  status: Status
  total: number | null
  notes: string
  hasCover: boolean
  created: string
}

export interface Session {
  id: string
  workId: string
  start: string | null
  finish: string | null
  progress: number
  completed: boolean
  rating: number | null
  review: string
  created: string
}

/** 手账整体由界面原样保存、原样回传，结构由 Rust 校验 */
export interface Journal {
  schema: number
  revision: number
  works: Work[]
  sessions: Session[]
  recentOps: string[]
  undo: { kind: string } | null
}

export type Fields = Pick<Work, 'kind' | 'title' | 'creator' | 'year' | 'genre' | 'tags' | 'total' | 'notes'>

export type Op =
  | { kind: 'createWork'; fields: Fields; status: Status }
  | { kind: 'editWork'; workId: string; fields: Fields }
  | { kind: 'setStatus'; workId: string; status: Status }
  | { kind: 'deleteWork'; workId: string }
  | { kind: 'start'; workId: string; start: string | null }
  | { kind: 'update'; sessionId: string; start: string | null; finish: string | null; progress: number; rating: number | null; review: string }
  | { kind: 'complete'; sessionId: string; finish: string | null }
  | { kind: 'uncomplete'; sessionId: string }
  | { kind: 'deleteSession'; sessionId: string }
  | { kind: 'setCover'; workId: string; hasCover: boolean }
  | { kind: 'undo' }

export interface Outcome {
  journal: Journal
  duplicate: boolean
  createdId: string | null
}

export type WorkView = Work & { sessions: number; completions: number; progress: number | null; bestRating: number | null; lastFinish: string | null }
export type SessionView = Session & { title: string; kind: Kind; total: number | null }

export interface View {
  revision: number
  works: WorkView[]
  total: number
  counts: { want: number; doing: number; done: number; paused: number; sessions: number }
  tags: string[]
  reviewYears: number[]
  pending: SessionView[]
  undo: string | null
}

export interface Review {
  year: number
  works: number
  completions: number
  books: number
  films: number
  pages: number
  minutes: number
  averageRating: number | null
  months: number[]
  items: { title: string; creator: string; kind: Kind; finish: string; stars: string; rating: number | null }[]
  pending: number
}

export interface Restored {
  journal: Journal | null
  covers: Record<string, string>
  removeCovers: string[]
  report: { worksAdded: number; sessionsAdded: number; worksSkipped: number; sessionsSkipped: number; covers: number; worksRemoved: number; sessionsRemoved: number }
}
