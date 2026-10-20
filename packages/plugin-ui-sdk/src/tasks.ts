import { Channel, invoke } from '@tauri-apps/api/core'
import type { LogLevel } from '@toolforge/ui'
import { toAppError, type AppError } from './types'

export type TaskStatus = 'running' | 'succeeded' | 'failed' | 'cancelled' | 'interrupted'

/** Rust 侧的结构化日志：code + params，由前端翻译 */
export interface LogLine {
  ts: number
  level: LogLevel
  code: string
  params?: Record<string, unknown>
}

export type TaskEvent =
  | { type: 'started'; taskId: string; pluginId: string; function: string; startedAt: number }
  | { type: 'logs'; taskId: string; lines: LogLine[] }
  | { type: 'progress'; taskId: string; done: number; total: number }
  | { type: 'stage'; taskId: string; code: string }
  | {
      type: 'finished'
      taskId: string
      status: Exclude<TaskStatus, 'running' | 'interrupted'>
      result: unknown
      error: AppError | null
      elapsedMs: number
    }

/** 与 Rust `TaskRecord` 对应 */
export interface TaskRecord {
  id: string
  pluginId: string
  function: string
  status: TaskStatus
  startedAt: number
  finishedAt: number | null
  elapsedMs: number | null
  errorCode: string | null
  /** 旧记录可能只有错误码；新任务保留用于错误文案插值的参数。 */
  errorParams?: Record<string, unknown>
  /** 保存了启动参数，可以重新运行 */
  rerunnable: boolean
}

/** 任务记录分页：当前页记录 + 总数（记录永久保留） */
export interface TaskPage {
  records: TaskRecord[]
  total: number
}

const wrap = async <T,>(promise: Promise<T>) => {
  try {
    return await promise
  } catch (error) {
    throw toAppError(error)
  }
}

export function startTask(pluginId: string, fn: string, args: object, onEvent: (event: TaskEvent) => void) {
  const channel = new Channel<TaskEvent>()
  channel.onmessage = onEvent
  return wrap(invoke<string>('task_start', { pluginId, function: fn, args, onEvent: channel }))
}

/** 以任务方式下载插件资源，事件格式与普通任务一致 */
export function startResourceDownload(pluginId: string, resourceId: string, onEvent: (event: TaskEvent) => void) {
  const channel = new Channel<TaskEvent>()
  channel.onmessage = onEvent
  return wrap(invoke<string>('resource_download', { pluginId, resourceId, onEvent: channel }))
}

/** 用保存的参数重新运行任务；进度通过全局任务事件显示在任务中心 */
export function rerunTask(taskId: string, onEvent: (event: TaskEvent) => void = () => {}) {
  const channel = new Channel<TaskEvent>()
  channel.onmessage = onEvent
  return wrap(invoke<string>('task_rerun', { taskId, onEvent: channel }))
}

/** 删除已结束的任务记录与日志 */
export const clearTasks = () => wrap(invoke<number>('task_clear'))

export const cancelTask = (taskId: string) => wrap(invoke<boolean>('task_cancel', { taskId }))
export const listTasks = (limit: number, offset: number) =>
  wrap(invoke<TaskPage>('task_list', { limit, offset }))
export const taskLogs = (taskId: string) => wrap(invoke<LogLine[]>('task_logs', { taskId }))
