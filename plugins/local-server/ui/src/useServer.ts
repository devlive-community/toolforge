import { useCallback, useEffect, useMemo, useState } from 'react'
import type { LogEntry } from '@toolforge/ui'
import { cancelTask, listTasks, taskLogs, useLogFormatter, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import type { Settings } from './types'

const POLL_MS = 1000

/**
 * 服务任务：本页启动的服务直接接收实时日志；离开工具后仍在运行的服务，
 * 回来时从任务列表中找回，并轮询日志文件与状态。
 */
export function useServer() {
  const { manifest } = usePlugin()
  const task = useTask()
  const ns = useMemo(() => [manifest.id, 'common'], [manifest.id])
  const format = useLogFormatter(ns)
  const [attached, setAttached] = useState<{ id: string; logs: LogEntry[] } | null>(null)

  useEffect(() => {
    let alive = true
    listTasks()
      .then((records) => {
        const running = records.find((r) => r.pluginId === manifest.id && r.function === 'serve' && r.status === 'running')
        if (alive && running) setAttached({ id: running.id, logs: [] })
      })
      .catch(() => {})
    return () => {
      alive = false
    }
  }, [manifest.id])

  const attachedId = attached?.id ?? null
  useEffect(() => {
    if (!attachedId) return
    let alive = true
    const poll = async () => {
      try {
        const [records, lines] = await Promise.all([listTasks(), taskLogs(attachedId)])
        if (!alive) return
        const record = records.find((r) => r.id === attachedId)
        if (!record || record.status !== 'running') {
          setAttached(null)
          return
        }
        setAttached({ id: attachedId, logs: lines.map((line, index) => ({ id: index, ts: line.ts, level: line.level, text: format(line) })) })
      } catch {
        // 下一轮再试
      }
    }
    poll()
    const timer = setInterval(poll, POLL_MS)
    return () => {
      alive = false
      clearInterval(timer)
    }
  }, [attachedId, format])

  const start = useCallback(
    (settings: Settings) => task.start('serve', settings),
    [task],
  )

  const stop = useCallback(async () => {
    if (task.running) await task.cancel()
    else if (attachedId) await cancelTask(attachedId)
  }, [task, attachedId])

  const running = task.running || attached !== null
  return {
    running,
    /** 找回的服务来自上一次打开工具 */
    reattached: !task.running && attached !== null,
    logs: task.running || !attached ? task.logs : attached.logs,
    error: task.status === 'failed' ? task.error : null,
    start,
    stop,
  }
}
