export type Kind = 'rows' | 'rounds'
export type Choice = 'execute' | 'skip'
// The full document is only transported to Rust and the host SQLite store.
export interface Workshop { schemaVersion: number; revision: number; activeProjectId: string | null; projects: unknown[] }
export interface Pages { projects: number; parts: number; segments: number; reminders: number; due: number; history: number }
export interface Page<T> { items: T[]; total: number; offset: number; limit: number }
export interface ProjectItem { id: string; name: string; partCount: number }
export interface PartItem { id: string; name: string; kind: Kind; completed: number; stitches: number }
export interface Pattern { id: string; name: string; index: number; start: number; end: number; row: number; repeat: number; repeats: number; rows: number; completedRepeats: number }
export interface SegmentItem { id: string; name: string; index: number; start: number; end: number; rows: number; repeats: number; completedRepeats: number; completedRows: number }
export interface ReminderItem { id: string; name: string; index: number; start: number; interval: number; repeats: number; delta: number; next: number | null; executed: number; skipped: number }
export interface DueItem { id: string; name: string; index: number; delta: number; occurrence: number; repeats: number; choice: Choice | null }
export interface HistoryItem { index: number; kind: 'line' | 'adjust'; completedBefore: number; completedAfter: number; stitchesBefore: number; stitchesAfter: number }
export interface PartView {
  id: string; name: string; kind: Kind; note: string; completed: number; nextLine: number; stitches: number; initialStitches: number
  canUndo: boolean; canEditRules: boolean; canComplete: boolean; pattern: Pattern | null; patternFinished: boolean
  dueCount: number; pendingCount: number; projectedStitches: number | null
  segments: Page<SegmentItem>; reminders: Page<ReminderItem>; due: Page<DueItem>; history: Page<HistoryItem>
}
export interface View { revision: number; projects: Page<ProjectItem>; parts: Page<PartItem>; activeProjectId: string | null; projectName: string | null; part: PartView | null; pages: Pages }
export interface Result { requestId: string; baseRevision: number; state: Workshop | null; view: View }
export type Op = { type: string } & Record<string, unknown>
export type FormKind = 'project' | 'part' | 'copy' | 'segment' | 'reminder' | 'details' | 'adjust'
export interface Exported { saved?: boolean; content?: string }
export const emptyWorkshop = (): Workshop => ({ schemaVersion: 1, revision: 0, activeProjectId: null, projects: [] })
export const emptyPages = (): Pages => ({ projects: 0, parts: 0, segments: 0, reminders: 0, due: 0, history: 0 })
