import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { LogEntry } from '@toolforge/ui'
import { useTranslation } from 'react-i18next'
import type { TFunction } from 'i18next'
import { usePlugin } from './context'
import { cancelTask, startTask, type LogLine, type TaskEvent, type TaskStatus } from './tasks'
import { toAppError, type AppError } from './types'

/** 内存中保留的最大日志行数（完整日志在 Rust 侧落盘） */
const MAX_LINES = 10_000

/** 翻译一条结构化日志：插件命名空间 logs.<code> → common logs.<code> → 原始 code */
export function formatLogLine(line: LogLine, t: TFunction, ns: string[]) {
  const key = `logs.${line.code}`
  const params = line.params ?? {}
  for (const namespace of ns) {
    if (t(key, { ns: namespace, defaultValue: '' })) return t(key, { ns: namespace, ...params })
  }
  const extra = Object.keys(params).length > 0 ? ` ${JSON.stringify(params)}` : ''
  return `${line.code}${extra}`
}

export function useLogFormatter(ns: string[]) {
  const { t } = useTranslation(ns)
  return useCallback((line: LogLine) => formatLogLine(line, t, ns), [t, ns])
}

export interface TaskState<T> {
  taskId: string | null
  status: 'idle' | TaskStatus
  logs: LogEntry[]
  progress: { done: number; total: number } | null
  stage: string | null
  result: T | null
  error: AppError | null
  elapsedMs: number | null
}

const initial: TaskState<never> = {
  taskId: null,
  status: 'idle',
  logs: [],
  progress: null,
  stage: null,
  result: null,
  error: null,
  elapsedMs: null,
}

/**
 * 在插件中运行耗时任务：Rust 通过 Channel 实时推送日志、进度与结果。
 */
export function useTask<T = unknown>() {
  const { manifest } = usePlugin()
  const ns = useMemo(() => [manifest.id, 'common'], [manifest.id])
  const format = useLogFormatter(ns)
  const [state, setState] = useState<TaskState<T>>(initial)
  const seq = useRef(0)
  const current = useRef<string | null>(null)
  const mounted = useRef(true)

  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])

  const onEvent = useCallback(
    (event: TaskEvent) => {
      if (!mounted.current || (current.current && event.taskId !== current.current)) return
      setState((prev) => {
        switch (event.type) {
          case 'logs': {
            const entries = event.lines.map((line) => ({ id: ++seq.current, ts: line.ts, level: line.level, text: format(line) }))
            const logs = prev.logs.length + entries.length > MAX_LINES
              ? [...prev.logs, ...entries].slice(-MAX_LINES)
              : [...prev.logs, ...entries]
            return { ...prev, logs }
          }
          case 'progress':
            return { ...prev, progress: { done: event.done, total: event.total } }
          case 'stage':
            return { ...prev, stage: event.code }
          case 'finished':
            return {
              ...prev,
              status: event.status,
              result: (event.result as T) ?? null,
              error: event.error,
              elapsedMs: event.elapsedMs,
            }
          default:
            return prev
        }
      })
    },
    [format],
  )

  const start = useCallback(
    async (fn: string, args: object = {}) => {
      current.current = null
      setState({ ...initial, status: 'running' })
      try {
        const taskId = await startTask(manifest.id, fn, args, onEvent)
        current.current = taskId
        setState((prev) => ({ ...prev, taskId }))
        return taskId
      } catch (error) {
        setState({ ...initial, status: 'failed', error: toAppError(error) })
        return null
      }
    },
    [manifest.id, onEvent],
  )

  const cancel = useCallback(async () => {
    if (current.current) await cancelTask(current.current)
  }, [])

  const reset = useCallback(() => {
    current.current = null
    setState(initial)
  }, [])

  return { ...state, running: state.status === 'running', start, cancel, reset }
}
