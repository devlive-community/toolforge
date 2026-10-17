import { useCallback, useEffect, useRef, useState } from 'react'
import { cancelTask, startTask, toAppError, usePlugin, type AppError, type TaskEvent } from '@toolforge/plugin-ui-sdk'
import { MAX_STREAM_EVENTS, type StreamBatch, type StreamItem, type StreamOpened } from './types'

export interface StreamRun {
  /** 正在连接或接收 */
  running: boolean
  opened: StreamOpened | null
  events: StreamItem[]
  batch: StreamBatch | null
  error: AppError | null
}

const idle: StreamRun = { running: false, opened: null, events: [], batch: null, error: null }

/** 以 Promise 形式运行任务；onStart 拿到任务 id 以便取消 */
function runTask<T>(pluginId: string, fn: string, args: object, onStart: (taskId: string) => void): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const onEvent = (event: TaskEvent) => {
      if (event.type !== 'finished') return
      if (event.status === 'succeeded') resolve(event.result as T)
      else reject(event.error ?? toAppError({ code: 'task.cancelled' }))
    }
    startTask(pluginId, fn, args, onEvent).then(onStart, (error) => reject(toAppError(error)))
  })
}

const append = (events: StreamItem[], added: StreamItem[]) => {
  if (added.length === 0) return events
  const next = [...events, ...added]
  return next.length > MAX_STREAM_EVENTS ? next.slice(next.length - MAX_STREAM_EVENTS) : next
}

/**
 * 流式接收：任务打开连接并返回第一批事件，之后短轮询取增量。
 * 新的连接、组件卸载都会关闭上一次的会话；generation 防止旧循环覆盖新结果。
 */
export function useStream() {
  const { manifest, call } = usePlugin()
  const [run, setRun] = useState<StreamRun>(idle)
  const generation = useRef(0)
  const taskId = useRef<string | null>(null)
  const session = useRef<number | null>(null)

  const release = useCallback(() => {
    if (taskId.current) void cancelTask(taskId.current).catch(() => {})
    if (session.current !== null) void call('stream_close', { session: session.current }).catch(() => {})
    taskId.current = null
    session.current = null
  }, [call])

  useEffect(() => () => {
    generation.current += 1
    release()
  }, [release])

  const start = useCallback(
    async (args: object) => {
      release()
      const gen = ++generation.current
      setRun({ ...idle, running: true })
      try {
        const opened = await runTask<StreamOpened>(manifest.id, 'stream', args, (id) => {
          taskId.current = id
        })
        taskId.current = null
        if (gen !== generation.current) {
          void call('stream_close', { session: opened.session }).catch(() => {})
          return
        }
        session.current = opened.session
        let events = append([], opened.events)
        let batch: StreamBatch = opened
        setRun({ running: !batch.done, opened, events, batch, error: null })
        while (!batch.done) {
          batch = await call<StreamBatch>('stream_poll', { session: opened.session })
          if (gen !== generation.current) return
          events = append(events, batch.events)
          setRun({ running: !batch.done, opened, events, batch, error: null })
        }
      } catch (error) {
        taskId.current = null
        if (gen !== generation.current) return
        const appError = toAppError(error)
        setRun((prev) => ({ ...prev, running: false, error: appError.code === 'task.cancelled' && !prev.opened ? null : appError }))
      }
    },
    [call, manifest.id, release],
  )

  const stop = useCallback(() => {
    if (taskId.current) void cancelTask(taskId.current).catch(() => {})
    else if (session.current !== null) void call('stream_stop', { session: session.current }).catch(() => {})
  }, [call])

  const exportEvents = useCallback(
    (format: 'json' | 'sse') => (session.current === null ? Promise.reject(toAppError({ code: 'http.stream_expired' })) : call<string>('stream_export', { session: session.current, format })),
    [call],
  )

  return { run, start, stop, exportEvents }
}
