// 闪卡数据模型：卡片与评分历史由 usePluginState 持久化（ToolForge SQLite）

/** 单次评分日志；prev 字段用于撤销恢复 */
export interface ReviewLogEntry {
  date: string
  remembered: boolean
  prevStage: number
  prevDue: string
  nextDue: string
}

export interface Card {
  id: string
  deck: string
  question: string
  answer: string
  tags: string[]
  paused: boolean
  /** 连续记住次数；0 且 due 为空 = 新卡 */
  stage: number
  /** "YYYY-MM-DD"，空串 = 新卡未安排 */
  due: string
  history: ReviewLogEntry[]
  createdAt: string
}

export interface FlashcardsState {
  cards: Card[]
  sessionLimit: number
  newLimit: number
  /** 最近一次评分（撤销一步） */
  lastUndo: { cardId: string; entry: ReviewLogEntry } | null
}

export interface ReviewOut {
  stage: number
  due: string
  interval: number
  prevStage: number
  prevDue: string
}

export interface QueueCard {
  id: string
  deck: string
  stage: number
  due: string
  paused: boolean
}

export interface QueueCounts {
  total: number
  paused: number
  due: number
  new: number
  selected: number
}

export interface QueueOut {
  dueIds: string[]
  newIds: string[]
  nextDue: string
  counts: QueueCounts
}

export interface ImportedCard {
  question: string
  answer: string
  deck: string
  tags: string[]
}

export interface RowError {
  line: number
  rule: string
}

export interface ImportOut {
  cards: ImportedCard[]
  errors: RowError[]
}

export interface BackupCard {
  id: string
  deck: string
  question: string
  answer: string
  tags: string[]
  paused: boolean
  stage: number
  due: string
  history: unknown[]
  createdAt: string
}

export interface ExportOut {
  json: string
  count: number
}

export interface Conflict {
  id: string
  question: string
}

export interface PreviewOut {
  incoming: number
  conflicts: Conflict[]
  fresh: number
}

export interface ApplyOut {
  cards: BackupCard[]
  overwritten: number
  skipped: number
}

/** 备份卡 → 运行时卡（history 类型收窄） */
export function toCard(backup: BackupCard): Card {
  return {
    id: backup.id,
    deck: backup.deck,
    question: backup.question,
    answer: backup.answer,
    tags: backup.tags,
    paused: backup.paused,
    stage: backup.stage,
    due: backup.due,
    history: backup.history as ReviewLogEntry[],
    createdAt: backup.createdAt,
  }
}

export function toBackupCard(card: Card): BackupCard {
  return {
    id: card.id,
    deck: card.deck,
    question: card.question,
    answer: card.answer,
    tags: card.tags,
    paused: card.paused,
    stage: card.stage,
    due: card.due,
    history: card.history,
    createdAt: card.createdAt,
  }
}
