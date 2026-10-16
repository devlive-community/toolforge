import type { AppError } from '@toolforge/plugin-ui-sdk'
import type { MarkdownBlock } from '@toolforge/ui'

export interface UpdateInfo {
  version: string
  currentVersion: string
  date: string | null
  notes: MarkdownBlock[]
}

export type UpdateEvent =
  | { type: 'progress'; downloaded: number; total: number | null }
  | { type: 'installing' }
  | { type: 'finished' }

export type UpdateStatus =
  | 'idle'
  | 'checking'
  | 'latest'
  | 'available'
  | 'downloading'
  | 'installing'
  | 'ready'
  | 'error'

export interface UpdateSnapshot {
  status: UpdateStatus
  info: UpdateInfo | null
  downloaded: number
  total: number | null
  error: AppError | null
  dialogOpen: boolean
  checkedAt: number | null
}

interface UpdateDependencies {
  get: () => UpdateSnapshot
  set: (state: Partial<UpdateSnapshot>) => void
  check: () => Promise<UpdateInfo | null>
  install: (onEvent: (event: UpdateEvent) => void) => Promise<unknown>
  skippedVersion: () => string | null
  toError: (error: unknown) => AppError
  now: () => number
}

const BUSY_STATUSES: UpdateStatus[] = ['checking', 'downloading', 'installing', 'ready']

export function createUpdateActions(deps: UpdateDependencies) {
  let attempt = 0

  return {
    /** silent：启动时的自动检查，不打扰用户（被跳过的版本不弹窗，失败不提示） */
    check: async ({ silent = false }: { silent?: boolean } = {}) => {
      if (BUSY_STATUSES.includes(deps.get().status)) return
      const currentAttempt = ++attempt
      deps.set({ status: 'checking', error: null })
      try {
        const info = await deps.check()
        if (currentAttempt !== attempt) return
        deps.set({
          info,
          status: info ? 'available' : 'latest',
          checkedAt: deps.now(),
          dialogOpen: info !== null && !(silent && info.version === deps.skippedVersion()),
        })
      } catch (error) {
        if (currentAttempt !== attempt) return
        deps.set({ status: silent ? 'idle' : 'error', error: deps.toError(error), checkedAt: deps.now() })
      }
    },

    install: async () => {
      const state = deps.get()
      if (!state.info || BUSY_STATUSES.includes(state.status)) return
      const currentAttempt = ++attempt
      deps.set({ status: 'downloading', downloaded: 0, total: null, error: null })
      try {
        await deps.install((event) => {
          if (currentAttempt !== attempt || !['downloading', 'installing'].includes(deps.get().status)) return
          if (event.type === 'progress') deps.set({ downloaded: event.downloaded, total: event.total })
          else if (event.type === 'installing') deps.set({ status: 'installing' })
          else deps.set({ status: 'ready' })
        })
        if (currentAttempt === attempt) deps.set({ status: 'ready' })
      } catch (error) {
        if (currentAttempt === attempt && deps.get().status !== 'ready') {
          deps.set({ status: 'error', error: deps.toError(error) })
        }
      } finally {
        // Channel 消息可能晚于命令完成；终结本次尝试后不再接受其事件。
        if (currentAttempt === attempt) attempt++
      }
    },
  }
}
