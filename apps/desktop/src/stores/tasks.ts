import { listen } from '@tauri-apps/api/event'
import { create } from 'zustand'
import { listTasks, type TaskEvent, type TaskRecord } from '@toolforge/plugin-ui-sdk'

interface Live {
  done: number
  total: number
  stage: string | null
}

interface TasksState {
  records: TaskRecord[]
  live: Record<string, Live>
  open: boolean
  setOpen: (open: boolean) => void
  load: () => Promise<void>
  apply: (event: TaskEvent) => void
}

export const useTasks = create<TasksState>((set, get) => ({
  records: [],
  live: {},
  open: false,
  setOpen: (open) => set({ open }),

  load: async () => set({ records: await listTasks() }),

  apply: (event) => {
    const { records, live } = get()
    switch (event.type) {
      case 'started':
        set({
          records: [
            {
              id: event.taskId,
              pluginId: event.pluginId,
              function: event.function,
              status: 'running',
              startedAt: event.startedAt,
              finishedAt: null,
              elapsedMs: null,
              errorCode: null,
            },
            ...records.filter((r) => r.id !== event.taskId),
          ],
        })
        break
      case 'progress':
        set({ live: { ...live, [event.taskId]: { stage: live[event.taskId]?.stage ?? null, done: event.done, total: event.total } } })
        break
      case 'stage':
        set({ live: { ...live, [event.taskId]: { done: live[event.taskId]?.done ?? 0, total: live[event.taskId]?.total ?? 0, stage: event.code } } })
        break
      case 'finished':
        set({
          records: records.map((r) =>
            r.id === event.taskId
              ? { ...r, status: event.status, elapsedMs: event.elapsedMs, finishedAt: Date.now(), errorCode: event.error?.code ?? null }
              : r,
          ),
        })
        break
    }
  },
}))

export const useRunningCount = () => useTasks((s) => s.records.filter((r) => r.status === 'running').length)

/** 订阅 Rust 广播的任务事件（不含日志），返回取消订阅函数 */
export function subscribeTasks(onFinished: (event: Extract<TaskEvent, { type: 'finished' }>) => void) {
  return listen<TaskEvent>('task://event', ({ payload }) => {
    useTasks.getState().apply(payload)
    if (payload.type === 'finished') onFinished(payload)
  })
}
