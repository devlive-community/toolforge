import { Channel, invoke } from '@tauri-apps/api/core'
import { create } from 'zustand'
import { toAppError, type AppError } from '@toolforge/plugin-ui-sdk'
import type { MarkdownBlock } from '@toolforge/ui'
import { usePrefs } from './prefs'

export interface UpdateInfo {
  version: string
  currentVersion: string
  date: string | null
  notes: MarkdownBlock[]
}

type UpdateEvent =
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

interface UpdateState {
  status: UpdateStatus
  info: UpdateInfo | null
  downloaded: number
  total: number | null
  error: AppError | null
  dialogOpen: boolean
  checkedAt: number | null
  /** silent：启动时的自动检查，不打扰用户（被跳过的版本不弹窗，失败不提示） */
  check: (options?: { silent?: boolean }) => Promise<void>
  install: () => Promise<void>
  restart: () => Promise<void>
  setDialogOpen: (open: boolean) => void
}

export const useUpdate = create<UpdateState>((set, get) => ({
  status: 'idle',
  info: null,
  downloaded: 0,
  total: null,
  error: null,
  dialogOpen: false,
  checkedAt: null,

  check: async ({ silent = false } = {}) => {
    if (['checking', 'downloading', 'installing'].includes(get().status)) return
    set({ status: 'checking', error: null })
    try {
      const info = await invoke<UpdateInfo | null>('update_check')
      const skipped = usePrefs.getState().skippedVersion
      set({
        info,
        status: info ? 'available' : 'latest',
        checkedAt: Date.now(),
        dialogOpen: info !== null && !(silent && info.version === skipped),
      })
    } catch (error) {
      set({ status: silent ? 'idle' : 'error', error: toAppError(error), checkedAt: Date.now() })
    }
  },

  install: async () => {
    const onEvent = new Channel<UpdateEvent>()
    onEvent.onmessage = (event) => {
      if (event.type === 'progress') set({ downloaded: event.downloaded, total: event.total })
      else if (event.type === 'installing') set({ status: 'installing' })
      else set({ status: 'ready' })
    }
    set({ status: 'downloading', downloaded: 0, total: null, error: null })
    try {
      await invoke('update_install', { onEvent })
    } catch (error) {
      set({ status: 'error', error: toAppError(error) })
    }
  },

  restart: () => invoke('app_restart'),
  setDialogOpen: (dialogOpen) => set({ dialogOpen }),
}))
