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
  total: number
  /** 运行中的任务数（由事件维护，与当前所在页无关） */
  running: number
  live: Record<string, Live>
  open: boolean
  setOpen: (open: boolean) => void
  /** 加载指定页（从 1 开始），替换当前页记录 */
  loadPage: (page: number) => Promise<void>
  apply: (event: TaskEvent) => void
}

/** 每页任务条数；记录全部常驻后端，页面按页读取 */
export const TASKS_PAGE = 50

export const useTasks = create<TasksState>((set, get) => ({
  records: [],
  total: 0,
  running: 0,
  live: {},
  open: false,
  setOpen: (open) => set({ open }),

  loadPage: async (page) => {
    const data = await listTasks(TASKS_PAGE, (page - 1) * TASKS_PAGE)
    set({ records: data.records, total: data.total })
  },

  apply: (event) => {
    const { records, total, running, live } = get()
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
              rerunnable: false,
            },
            ...records.filter((r) => r.id !== event.taskId),
          ],
          total: total + 1,
          running: running + 1,
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
              ? {
                  ...r,
                  status: event.status,
                  elapsedMs: event.elapsedMs,
                  finishedAt: Date.now(),
                  errorCode: event.error?.code ?? null,
                  errorParams: event.error?.params,
                }
              : r,
          ),
          running: Math.max(0, running - 1),
        })
        break
    }
  },
}))

export const useRunningCount = () => useTasks((s) => s.running)

/** 订阅 Rust 广播的任务事件（不含日志），返回取消订阅函数 */
export function subscribeTasks(onFinished: (event: Extract<TaskEvent, { type: 'finished' }>) => void) {
  return listen<TaskEvent>('task://event', ({ payload }) => {
    useTasks.getState().apply(payload)
    if (payload.type === 'finished') onFinished(payload)
  })
}
