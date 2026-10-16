import { Channel, invoke } from '@tauri-apps/api/core'
import { create } from 'zustand'
import { toAppError } from '@toolforge/plugin-ui-sdk'
import { usePrefs } from './prefs'
import { createUpdateActions, type UpdateEvent, type UpdateInfo, type UpdateSnapshot } from './updateActions'

export type { UpdateInfo, UpdateStatus } from './updateActions'

interface UpdateState extends UpdateSnapshot {
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

  ...createUpdateActions({
    get,
    set,
    check: () => invoke<UpdateInfo | null>('update_check'),
    install: (onmessage) => {
      const onEvent = new Channel<UpdateEvent>()
      onEvent.onmessage = onmessage
      return invoke('update_install', { onEvent })
    },
    skippedVersion: () => usePrefs.getState().skippedVersion,
    toError: toAppError,
    now: Date.now,
  }),

  restart: () => invoke('app_restart'),
  setDialogOpen: (dialogOpen) => set({ dialogOpen }),
}))
