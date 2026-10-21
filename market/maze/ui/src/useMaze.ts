import { useEffect, useRef, useState } from 'react'
import { loadPluginState, savePluginState, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { emptyState, type Op, type Result, type State, type View } from './types'

export function useMaze() {
  const { manifest, errorMessage } = usePlugin()
  const task = useTask<Result>()
  const [view, setView] = useState<View | null>(null)
  const [busy, setBusy] = useState(true)
  const [saveError, setSaveError] = useState<string | null>(null)
  const [notice, setNotice] = useState('ready')
  const [status, setStatus] = useState('loading')
  const source = useRef<State | null>(null)
  const locked = useRef(false)
  const failedSave = useRef<Result | null>(null)
  const sequence = useRef(0)
  const current = useRef('')
  const handled = useRef('')
  const mounted = useRef(true)
  const waiter = useRef<{ resolve: () => void; reject: (error: unknown) => void } | null>(null)
  const start = useRef(task.start)
  const translate = useRef(errorMessage)
  const commitRef = useRef<(result: Result) => Promise<void>>(async () => {})
  const reloadRef = useRef<() => Promise<void>>(async () => {})

  const run = (op?: Op): Promise<void> => {
    if (locked.current || failedSave.current || !source.current) return Promise.reject({ code: 'maze.busy' })
    locked.current = true
    setBusy(true)
    setStatus(op ? 'saving' : 'loading')
    const id = String(++sequence.current)
    current.current = id
    return new Promise<void>((resolve, reject) => {
      waiter.current = { resolve, reject }
      void start.current(op ? 'apply' : 'inspect', { state: source.current, requestId: id, op }).then((taskId) => {
        if (!taskId) {
          locked.current = false
          setBusy(false)
          waiter.current?.reject({ code: 'maze.operation' })
          waiter.current = null
        }
      })
    })
  }
  const commit = async (result: Result) => {
    try {
      if (source.current?.revision !== result.baseRevision) throw { code: 'maze.stale' }
      if (result.state) await savePluginState(manifest.id, 'progress', result.state)
      if (!mounted.current) return
      if (result.state) source.current = result.state
      failedSave.current = null
      setSaveError(null)
      setView(result.view)
      setNotice(result.notice)
      setStatus(result.state ? 'saved' : 'ready')
      waiter.current?.resolve()
      waiter.current = null
    } catch (error) {
      if (mounted.current) {
        failedSave.current = result
        setSaveError(translate.current(error))
        setStatus('error')
      }
    } finally {
      locked.current = false
      if (mounted.current) setBusy(false)
    }
  }
  const reload = async () => {
    if (locked.current) return
    locked.current = true
    setBusy(true)
    setSaveError(null)
    setStatus('loading')
    try {
      const state = await loadPluginState<State>(manifest.id, 'progress')
      if (!mounted.current) return
      source.current = state ?? emptyState()
      locked.current = false
      await run()
    } catch (error) {
      if (mounted.current) {
        locked.current = false
        setBusy(false)
        setStatus('error')
        setSaveError(translate.current(error))
      }
    }
  }
  const retry = () => {
    if (locked.current) return
    if (failedSave.current) {
      locked.current = true
      setBusy(true)
      setStatus('saving')
      void commitRef.current(failedSave.current)
    } else void reloadRef.current()
  }
  useEffect(() => {
    start.current = task.start
    translate.current = errorMessage
    commitRef.current = commit
    reloadRef.current = reload
  })
  useEffect(() => {
    mounted.current = true
    void reloadRef.current()
    return () => {
      mounted.current = false
      waiter.current?.reject({ code: 'task.cancelled' })
      waiter.current = null
    }
  }, [manifest.id])
  useEffect(() => {
    const result = task.result
    if (!result || result.requestId !== current.current || handled.current === result.requestId) return
    handled.current = result.requestId
    void Promise.resolve().then(() => commitRef.current(result))
  }, [task.result])
  useEffect(() => {
    if (!['failed', 'cancelled'].includes(task.status)) return
    void Promise.resolve().then(() => {
      if (!mounted.current) return
      locked.current = false
      setBusy(false)
      setStatus(task.status === 'cancelled' ? 'ready' : 'error')
      if (task.status === 'cancelled') setNotice('cancelled')
      waiter.current?.reject(task.error ?? { code: 'task.cancelled' })
      waiter.current = null
    })
  }, [task.status, task.error])
  return { view, busy, saveError, notice, status, run, retry, source, task }
}
