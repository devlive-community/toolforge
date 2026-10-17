import { useEffect, useRef, useState } from 'react'
import { Button, CodeEditor, Empty, Input, NumberInput, Panel, Switch, toast } from '@toolforge/ui'
import { host, loadPluginState, savePluginState, usePlugin } from '@toolforge/plugin-ui-sdk'
import { CheckCheck, FileJson, KeyRound, Play, RotateCcw, Square, TriangleAlert } from 'lucide-react'
import { MetadataModal } from './MetadataModal'
import { ResponsePanel } from './ResponsePanel'
import { cancel as cancelTask, runTask } from './runTask'
import { SourcePanel } from './SourcePanel'
import { defaultSettings, type Batch, type Loaded, type Method, type Run, type Settings, type SourceKind, type Started } from './types'

const STATE_KEY = 'settings-v1'
const SAVE_DELAY_MS = 500
/** 保存的请求草稿上限 */
const MAX_DRAFTS = 100

export default function GrpcClient() {
  const { t, manifest, call, errorMessage } = usePlugin()
  const [settings, setSettings] = useState<Settings>(defaultSettings)
  const current = useRef(settings)
  const saveTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  const [loaded, setLoaded] = useState<Loaded | null>(null)
  const [loading, setLoading] = useState(false)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [method, setMethod] = useState<Method | null>(null)
  const [json, setJson] = useState('')
  const [checked, setChecked] = useState<{ ok: boolean; text: string } | null>(null)
  const [run, setRun] = useState<Run | null>(null)
  const [callError, setCallError] = useState<string | null>(null)
  const [metadataOpen, setMetadataOpen] = useState(false)
  const generation = useRef(0)
  const taskId = useRef<string | null>(null)
  const callId = useRef<number | null>(null)

  /** 先由 Rust 清空凭据的值再保存 */
  const persist = (next: Settings) => {
    clearTimeout(saveTimer.current)
    saveTimer.current = setTimeout(() => {
      call<Settings>('sanitize', next)
        .then((clean) => savePluginState(manifest.id, STATE_KEY, clean))
        .catch((error) => toast.error(errorMessage(error)))
    }, SAVE_DELAY_MS)
  }
  const update = (patch: Partial<Settings>) => {
    const next = { ...current.current, ...patch }
    current.current = next
    setSettings(next)
    persist(next)
  }

  const selectMethod = (next: Method, token: number) => {
    setMethod(next)
    setChecked(null)
    update({ method: next.path })
    const draft = current.current.drafts[next.path]
    if (draft !== undefined) {
      setJson(draft)
      return
    }
    call<{ json: string }>('template', { token, method: next.path }).then(
      (out) => setJson(out.json),
      (error) => toast.error(errorMessage(error)),
    )
  }

  const load = (kind: SourceKind) => {
    const s = current.current
    const [fn, args] =
      kind === 'proto'
        ? ['load_proto', { path: s.source.proto, includes: s.source.includes }]
        : kind === 'descriptor'
          ? ['load_descriptor', { path: s.source.descriptor }]
          : ['reflect', { target: s.target, metadata: s.metadata }]
    setLoading(true)
    setLoadError(null)
    runTask<Loaded>(manifest.id, fn, args)
      .then((result) => {
        setLoaded(result)
        const methods = result.services.flatMap((service) => service.methods)
        const remembered = methods.find((m) => m.path === current.current.method)
        if (remembered) selectMethod(remembered, result.token)
        else {
          setMethod(null)
          setJson('')
        }
      })
      .catch((error) => setLoadError(errorMessage(error)))
      .finally(() => setLoading(false))
  }

  const loadRef = useRef(load)
  useEffect(() => {
    loadRef.current = load
  })

  // 读取保存的设置；本地定义来源离线可用，打开工具时自动重新加载
  useEffect(() => {
    let alive = true
    loadPluginState<Settings>(manifest.id, STATE_KEY)
      .then((saved) => {
        if (!alive || !saved) return
        const next = { ...defaultSettings(), ...saved, source: { ...defaultSettings().source, ...saved.source } }
        current.current = next
        setSettings(next)
        const { kind, proto, descriptor } = next.source
        if ((kind === 'proto' && proto) || (kind === 'descriptor' && descriptor)) loadRef.current(kind)
      })
      .catch(() => {})
    return () => {
      alive = false
    }
  }, [manifest.id])

  // 离开工具时结束进行中的调用
  useEffect(
    () => () => {
      generation.current += 1
      void cancelTask(taskId.current)
      if (callId.current !== null) void call('cancel', { call: callId.current }).catch(() => {})
      clearTimeout(saveTimer.current)
    },
    [call],
  )

  const editJson = (text: string) => {
    setJson(text)
    setChecked(null)
    if (!method) return
    const drafts = { ...current.current.drafts, [method.path]: text }
    const keys = Object.keys(drafts)
    if (keys.length > MAX_DRAFTS) delete drafts[keys[0]]
    update({ drafts })
  }

  const resetDraft = () => {
    if (!method || !loaded) return
    call<{ json: string }>('template', { token: loaded.token, method: method.path }).then(
      (out) => editJson(out.json),
      (error) => toast.error(errorMessage(error)),
    )
  }

  const validate = () => {
    if (!method || !loaded) return
    call<{ bytes: number }>('validate', { token: loaded.token, method: method.path, json }).then(
      (out) => setChecked({ ok: true, text: t('request.valid', { bytes: out.bytes }) }),
      (error) => setChecked({ ok: false, text: errorMessage(error) }),
    )
  }

  const invoke = async () => {
    if (!method || !loaded || run?.running) return
    const gen = ++generation.current
    setCallError(null)
    setRun({ call: null, running: true, headers: [], messages: [], batch: null })
    const s = current.current
    try {
      const started = await runTask<Started>(
        manifest.id,
        'invoke',
        {
          token: loaded.token,
          method: method.path,
          target: s.target,
          metadata: s.metadata,
          deadlineMs: Math.round(s.deadlineSec * 1000),
          json,
        },
        (id) => {
          taskId.current = id
        },
      )
      taskId.current = null
      if (gen !== generation.current) return
      const id = started.call || null
      callId.current = id
      let messages = started.messages
      let batch: Batch = started
      setRun({ call: id, running: !batch.done, headers: started.headers, messages, batch })
      while (!batch.done && id !== null) {
        batch = await call<Batch>('poll', { call: id, waitMs: 1000 })
        if (gen !== generation.current) return
        messages = [...messages, ...batch.messages]
        setRun({ call: id, running: !batch.done, headers: started.headers, messages, batch })
      }
      callId.current = null
    } catch (error) {
      taskId.current = null
      callId.current = null
      if (gen !== generation.current) return
      setRun(null)
      const appError = error as { code?: string }
      if (appError?.code !== 'task.cancelled') setCallError(errorMessage(error))
    }
  }

  const stop = () => {
    if (taskId.current) void cancelTask(taskId.current)
    else if (callId.current !== null) void call('cancel', { call: callId.current }).catch(() => {})
  }

  const exportResult = async (copy: boolean) => {
    if (!run?.call) return
    try {
      const text = await call<string>('export', { call: run.call })
      if (copy) {
        await host.clipboard.writeText(text)
        toast.success(t('export.copied'))
        return
      }
      const path = await host.dialog.saveFile('grpc-response.json', [{ name: 'JSON', extensions: ['json'] }])
      if (!path) return
      await host.fs.writeText(path, text)
      toast.success(t('export.saved'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const activeMetadata = settings.metadata.filter((m) => m.enabled && m.key.trim()).length
  const running = !!run?.running

  return (
    <div className="grid h-full min-h-0 grid-cols-[300px_minmax(0,1fr)] gap-3">
      <SourcePanel
        settings={settings}
        onSettings={update}
        loaded={loaded}
        loading={loading}
        error={loadError}
        onLoad={load}
        selected={method?.path ?? ''}
        onSelect={(m) => loaded && selectMethod(m, loaded.token)}
      />
      <div className="flex min-h-0 flex-col gap-3">
        <form
          className="flex shrink-0 flex-wrap items-center gap-2 rounded-card border border-border bg-surface p-3 shadow-card"
          onSubmit={(event) => {
            event.preventDefault()
            void invoke()
          }}
        >
          <Input
            value={settings.target.address}
            onChange={(e) => update({ target: { ...settings.target, address: e.target.value } })}
            placeholder="localhost:50051"
            className="font-mono"
            wrapperClassName="min-w-48 flex-1"
            aria-label={t('target.address')}
          />
          <label className="flex items-center gap-1.5 text-xs text-fg-muted">
            <Switch checked={settings.target.tls} onCheckedChange={(tls) => update({ target: { ...settings.target, tls } })} aria-label={t('target.tls')} />
            {t('target.tls')}
          </label>
          <label className="flex items-center gap-1.5 text-xs text-fg-muted">
            {t('target.deadline')}
            <NumberInput size="sm" value={settings.deadlineSec} onValueChange={(deadlineSec) => update({ deadlineSec })} min={1} max={3600} className="w-24" aria-label={t('target.deadline')} />
          </label>
          <Button type="button" variant="outline" onClick={() => setMetadataOpen(true)}>
            <KeyRound />
            {t('metadata.button', { count: activeMetadata })}
          </Button>
          {running ? (
            <Button type="button" onClick={stop}>
              <Square />
              {t('call.cancel')}
            </Button>
          ) : (
            <Button type="submit" variant="primary" disabled={!method?.supported || !settings.target.address.trim()}>
              <Play />
              {t('call.invoke')}
            </Button>
          )}
        </form>
        {callError && (
          <p className="flex items-start gap-1.5 text-xs break-words text-danger" role="alert">
            <TriangleAlert className="mt-0.5 size-3.5 shrink-0" />
            {callError}
          </p>
        )}
        <div className="grid min-h-0 flex-1 grid-cols-1 gap-3 xl:grid-cols-2">
          <Panel
            title={method ? <span className="truncate font-mono">{method.path}</span> : t('request.title')}
            icon={<FileJson />}
            extra={method && <span className="truncate text-xs text-fg-subtle">{t('request.types', { input: method.input, output: method.output })}</span>}
            actions={
              method && (
                <>
                  <Button size="sm" variant="ghost" onClick={resetDraft}>
                    <RotateCcw />
                    {t('request.reset')}
                  </Button>
                  <Button size="sm" variant="ghost" onClick={validate}>
                    <CheckCheck />
                    {t('request.validate')}
                  </Button>
                </>
              )
            }
            footer={
              checked ? (
                <span className={checked.ok ? 'text-success' : 'truncate text-danger'}>{checked.text}</span>
              ) : method && !method.supported ? (
                <span className="text-warning">{t('request.unsupported')}</span>
              ) : method?.serverStreaming ? (
                <span>{t('request.streaming')}</span>
              ) : undefined
            }
            className="min-h-0"
            bodyClassName="min-h-0"
          >
            {method ? (
              <CodeEditor value={json} onChange={editJson} language="json" aria-label={t('request.title')} />
            ) : (
              <Empty icon={<FileJson />} title={t('request.empty')} description={t('request.emptyHint')} className="h-full" />
            )}
          </Panel>
          <ResponsePanel run={run} onExport={(copy) => void exportResult(copy)} />
        </div>
      </div>
      <MetadataModal open={metadataOpen} onClose={() => setMetadataOpen(false)} entries={settings.metadata} onChange={(metadata) => update({ metadata })} />
    </div>
  )
}
