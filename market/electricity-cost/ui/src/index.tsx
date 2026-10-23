import { useEffect, useRef, useState } from 'react'
import { host, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { Badge, Button, LogViewer, Panel, SegmentedControl, Select, Spinner, Tooltip, cn, toast } from '@toolforge/ui'
import { Calculator, Copy, Download, Plus, Square, Trash2, Zap } from 'lucide-react'
import { Appliances, Field, TariffEditor } from './Editors'
import { Library } from './Library'
import { Results } from './Results'
import type { Appliance, Input, Report, Tariff } from './types'
export default function ElectricityCost() {
  const { t, call, errorMessage } = usePlugin()
  const task = useTask<Report>()
  const exporting = useTask<{ content?: string; saved?: boolean }>()
  const [input, setInput] = useState<Input | null>(null)
  const [report, setReport] = useState<Report | null>(null)
  const [initError, setInitError] = useState<string | null>(null)
  const [selected, setSelected] = useState(0)
  const [dirty, setDirty] = useState(false)
  const [pane, setPane] = useState<'input' | 'result'>('input')
  const [tab, setTab] = useState<'appliances' | 'tariffs'>('appliances')
  const [extraBusy, setExtraBusy] = useState(false)
  const [libraryBusy, setLibraryBusy] = useState(false)
  const handled = useRef('')
  const api = useRef({ call, errorMessage, t })
  useEffect(() => {
    api.current = { call, errorMessage, t }
  })
  useEffect(() => {
    let alive = true
    void api.current
      .call<Input>('defaults')
      .then((value) => {
        if (alive)
          setInput({
            ...value,
            tariffs: value.tariffs.map((v) => ({ ...v, name: api.current.t('tariff.exampleName') })),
            appliances: value.appliances.map((v) => ({ ...v, name: api.current.t('presets.lighting') })),
          })
      })
      .catch((e) => {
        if (alive) setInitError(api.current.errorMessage(e))
      })
    return () => {
      alive = false
    }
  }, [])
  useEffect(() => {
    if (task.result) {
      const result = task.result
      void Promise.resolve().then(() => {
        setReport(result)
        setDirty(false)
      })
    }
  }, [task.result])
  useEffect(() => {
    const result = exporting.result
    if (!result || !exporting.taskId || handled.current === exporting.taskId) return
    handled.current = exporting.taskId
    if (result.content)
      void host.clipboard
        .writeText(result.content)
        .then(() => toast.success(api.current.t('export.copied')))
        .catch((e) => toast.error(api.current.errorMessage(e)))
  }, [exporting.result, exporting.taskId])
  const busy = task.running || exporting.running || extraBusy || libraryBusy
  const update = (next: Input) => {
    setInput(next)
    setDirty(true)
    exporting.reset()
  }
  const add = async (preset: string) => {
    if (!input || busy) return
    setExtraBusy(true)
    try {
      const a = await call<Appliance>('new', { kind: 'appliance', preset })
      update({ ...input, appliances: [{ ...a, name: t(`presets.${preset}`) }, ...input.appliances] })
    } catch (e) {
      toast.error(errorMessage(e))
    } finally {
      setExtraBusy(false)
    }
  }
  const addTariff = async () => {
    if (!input || busy) return
    setExtraBusy(true)
    try {
      const a = await call<Tariff>('new', { kind: 'tariff' })
      update({ ...input, tariffs: [...input.tariffs, { ...a, name: t('tariff.newName') }] })
      setSelected(input.tariffs.length)
    } catch (e) {
      toast.error(errorMessage(e))
    } finally {
      setExtraBusy(false)
    }
  }
  const run = async () => {
    if (!input || busy) return
    exporting.reset()
    const id = await task.start('calculate', input)
    if (id !== null) setPane('result')
  }
  const exportReport = async (format: 'json' | 'markdown', copy = false) => {
    if (!report || busy) return
    setExtraBusy(true)
    try {
      const ext = format === 'json' ? 'json' : 'md'
      const path = copy ? undefined : await host.dialog.saveFile(`electricity-estimate.${ext}`, [{ name: t(`export.${format}`), extensions: [ext] }])
      if (path === null) return
      await exporting.start('export', { handle: report.handle, format, language: t('locale'), path })
    } catch (e) {
      toast.error(errorMessage(e))
    } finally {
      setExtraBusy(false)
    }
  }
  const cancel = async () => {
    try {
      await (task.running ? task.cancel() : exporting.cancel())
    } catch (e) {
      toast.error(errorMessage(e))
    }
  }
  if (!input) return <div className="flex h-full items-center justify-center text-sm text-danger">{initError ?? <Spinner />}</div>
  const tariff = input.tariffs[selected] ?? input.tariffs[0]
  return (
    <div className="@container flex h-full min-h-0 flex-col gap-3">
      <div className="flex shrink-0 flex-wrap items-center gap-2">
        <Button size="sm" variant="primary" disabled={busy} onClick={() => void run()}>
          <Calculator />
          {t('calculate')}
        </Button>
        {(task.running || exporting.running) && (
          <Button size="sm" onClick={() => void cancel()}>
            <Square />
            {t('common.cancel')}
          </Button>
        )}
        <Badge>{t('offline')}</Badge>
        <span className="flex-1" />
        {report && (
          <>
            <Tooltip content={t('export.copy')}>
              <Button size="icon-sm" disabled={busy} aria-label={t('export.copy')} onClick={() => void exportReport('markdown', true)}>
                <Copy />
              </Button>
            </Tooltip>
            {(['json', 'markdown'] as const).map((format) => (
              <Button key={format} size="sm" disabled={busy} onClick={() => void exportReport(format)}>
                <Download />
                {t(`export.${format}`)}
              </Button>
            ))}
          </>
        )}
      </div>
      {(task.error || exporting.error) && (
        <p role="alert" className="shrink-0 rounded-control bg-danger/5 p-2 text-xs text-danger">
          {errorMessage(task.error ?? exporting.error)}
        </p>
      )}
      {exporting.result?.saved && (
        <p role="status" className="text-xs text-success">
          {t('export.saved')}
        </p>
      )}
      {dirty && report && <p className="text-[11px] text-warning">{t('stale')}</p>}
      <div className="shrink-0 @min-[850px]:hidden">
        <SegmentedControl
          value={pane}
          onValueChange={setPane}
          size="sm"
          aria-label={t('panes.label')}
          options={[
            { value: 'input', label: t('panes.input') },
            { value: 'result', label: t('panes.result') },
          ]}
        />
      </div>
      <div className="grid min-h-0 flex-1 grid-cols-1 gap-3 @min-[850px]:grid-cols-[minmax(350px,0.95fr)_minmax(0,1.05fr)]">
        <Panel
          className={cn('min-h-0', pane !== 'input' && 'hidden @min-[850px]:flex')}
          title={t('input.title')}
          icon={<Zap />}
          bodyClassName="h-full min-h-0 space-y-4 overflow-auto p-3"
        >
          <fieldset disabled={busy} className="space-y-4 disabled:opacity-60">
            <div className="grid grid-cols-2 gap-3">
              <Field label={t('input.days')} value={String(input.days)} change={(v) => update({ ...input, days: Number(v) })} />
              <Field label={t('input.currency')} value={input.currency} maxLength={12} change={(currency) => update({ ...input, currency })} />
            </div>
            <SegmentedControl
              size="sm"
              value={tab}
              onValueChange={setTab}
              aria-label={t('input.sections')}
              options={[
                { value: 'appliances', label: t('appliances.title', { n: input.appliances.length }) },
                { value: 'tariffs', label: t('tariff.title', { n: input.tariffs.length }) },
              ]}
            />
            {tab === 'appliances' ? (
              <Appliances
                values={input.appliances}
                disabled={busy}
                add={(preset) => void add(preset)}
                change={(appliances) => update({ ...input, appliances })}
              />
            ) : (
              <div className="space-y-4">
                <div className="flex gap-2">
                  <div className="min-w-0 flex-1">
                    <Select
                      size="sm"
                      value={String(selected)}
                      onValueChange={(v) => setSelected(Number(v))}
                      aria-label={t('tariff.choose')}
                      options={input.tariffs.map((p, i) => ({ value: String(i), label: p.name }))}
                    />
                  </div>
                  <Tooltip content={t('tariff.add')}>
                    <Button size="icon-sm" disabled={busy || input.tariffs.length >= 8} aria-label={t('tariff.add')} onClick={() => void addTariff()}>
                      <Plus />
                    </Button>
                  </Tooltip>
                  <Tooltip content={t('tariff.remove')}>
                    <Button
                      size="icon-sm"
                      disabled={busy || input.tariffs.length <= 1}
                      aria-label={t('tariff.remove')}
                      onClick={() => {
                        update({ ...input, tariffs: input.tariffs.filter((p) => p.id !== tariff.id) })
                        setSelected(0)
                      }}
                    >
                      <Trash2 />
                    </Button>
                  </Tooltip>
                </div>
                <p className="text-[11px] text-warning">{t('tariff.example')}</p>
                <TariffEditor tariff={tariff} change={(next) => update({ ...input, tariffs: input.tariffs.map((p) => (p.id === next.id ? next : p)) })} />
              </div>
            )}
          </fieldset>
          {tab === 'tariffs' && (
            <Library
              onBusy={setLibraryBusy}
              tariff={tariff}
              disabled={busy}
              apply={(saved, append) => {
                const existing = input.tariffs.findIndex((p) => p.id === saved.id)
                if (existing >= 0) {
                  update({ ...input, tariffs: input.tariffs.map((p) => (p.id === saved.id ? saved : p)) })
                  setSelected(existing)
                } else if (append) {
                  if (input.tariffs.length >= 8) {
                    toast.error(t('library.compareLimit'))
                    return
                  }
                  update({ ...input, tariffs: [...input.tariffs, saved] })
                  setSelected(input.tariffs.length)
                } else update({ ...input, tariffs: input.tariffs.map((p) => (p.id === tariff.id ? saved : p)) })
              }}
            />
          )}
          <p className="text-[11px] leading-relaxed text-fg-subtle">{t('input.boundary')}</p>
        </Panel>
        <div className={cn('flex min-h-0 flex-col', pane !== 'result' && 'hidden @min-[850px]:flex')}>
          <Results key={report?.handle ?? 'empty'} report={report} />
        </div>
      </div>
      {(task.logs.length > 0 || exporting.logs.length > 0) && (
        <details className="shrink-0 rounded-control border border-border">
          <summary className="cursor-pointer p-2 text-[11px] text-fg-muted">{t('logs.title')}</summary>
          <LogViewer entries={exporting.running ? exporting.logs : task.logs} className="h-24" emptyText={t('logs.empty')} jumpLabel={t('logs.latest')} />
        </details>
      )}
    </div>
  )
}
