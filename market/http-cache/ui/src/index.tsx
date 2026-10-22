import { useEffect, useRef, useState } from 'react'
import { Button, Empty, LogViewer, Panel, SegmentedControl, Select, Spinner, Tooltip, toast } from '@toolforge/ui'
import { host, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { Copy, Download, FileJson, Gauge, Square } from 'lucide-react'
import { InputForm } from './InputForm'
import { Cards, Detail } from './Results'
import type { Exported, Inspected, Scenario, Section, View } from './types'

export default function HttpCache() {
  const { t, call, errorMessage } = usePlugin()
  const analysis = useTask<Inspected>()
  const query = useTask<View>()
  const exporting = useTask<Exported>()
  const [cases, setCases] = useState<Scenario[] | null>(null)
  const [inputCase, setInputCase] = useState(0)
  const [compare, setCompare] = useState(true)
  const [inspection, setInspection] = useState<Inspected | null>(null)
  const [view, setView] = useState<View | null>(null)
  const [section, setSection] = useState<Section>('conditions')
  const [busy, setBusy] = useState(false)
  const [dirty, setDirty] = useState(false)
  const [pane, setPane] = useState<'input' | 'result'>('input')
  const [loadError, setLoadError] = useState<string | null>(null)
  const pending = useRef(false)
  const requestedSection = useRef<Section>('conditions')
  const handled = useRef('')
  const handledQuery = useRef('')
  const handledExport = useRef('')
  const mounted = useRef(true)
  const queryStart = useRef(query.start)
  const callRef = useRef(call)
  const errorRef = useRef(errorMessage)
  useEffect(() => {
    queryStart.current = query.start
    callRef.current = call
    errorRef.current = errorMessage
  })
  useEffect(() => {
    mounted.current = true
    callRef
      .current<Scenario[]>('examples')
      .then((items) => {
        if (mounted.current) setCases(items)
      })
      .catch((error) => {
        if (mounted.current) setLoadError(errorRef.current(error))
      })
    return () => {
      mounted.current = false
    }
  }, [])
  const finish = () => {
    pending.current = false
    setBusy(false)
  }
  const inspect = async () => {
    if (pending.current || !cases) return
    pending.current = true
    setBusy(true)
    const id = await analysis.start('inspect', { cases: compare ? cases : [cases[inputCase]] })
    if (id === null) finish()
  }
  const request = async (index: number, target: Section, offset = 0) => {
    if (pending.current || !inspection) return
    pending.current = true
    setBusy(true)
    requestedSection.current = target
    const id = await query.start('query', {
      handle: inspection.handle,
      case: index,
      section: target === 'timing' || target === 'headers' ? 'conditions' : target,
      offset,
      limit: 20,
    })
    if (id === null) finish()
  }
  const save = async (format: 'json' | 'markdown', file: boolean) => {
    if (pending.current || !inspection) return
    pending.current = true
    setBusy(true)
    exporting.reset()
    try {
      const ext = format === 'json' ? 'json' : 'md'
      const path = file ? await host.dialog.saveFile(`http-cache-report.${ext}`, [{ name: ext.toUpperCase(), extensions: [ext] }]) : null
      if (file && !path) {
        finish()
        return
      }
      const id = await exporting.start('export', { handle: inspection.handle, format, language: t('locale'), path })
      if (id === null) finish()
    } catch (error) {
      toast.error(errorMessage(error))
      finish()
    }
  }
  useEffect(() => {
    const result = analysis.result
    if (!result || result.handle === handled.current) return
    handled.current = result.handle
    Promise.resolve().then(async () => {
      if (!mounted.current) return
      setInspection(result)
      setView(null)
      setDirty(false)
      setPane('result')
      requestedSection.current = 'conditions'
      const id = await queryStart.current('query', { handle: result.handle, case: 0, section: 'conditions', offset: 0, limit: 20 })
      if (id === null && mounted.current) {
        pending.current = false
        setBusy(false)
      }
    })
  }, [analysis.result])
  useEffect(() => {
    const id = query.taskId
    if (!id || query.running || query.status === 'idle' || handledQuery.current === id) return
    handledQuery.current = id
    Promise.resolve().then(() => {
      if (!mounted.current) return
      if (query.result) {
        setView(query.result)
        setSection(requestedSection.current)
      }
      pending.current = false
      setBusy(false)
    })
  }, [query.taskId, query.running, query.status, query.result])
  useEffect(() => {
    if (analysis.status !== 'failed' && analysis.status !== 'cancelled') return
    Promise.resolve().then(() => {
      if (mounted.current) {
        pending.current = false
        setBusy(false)
      }
    })
  }, [analysis.status])
  useEffect(() => {
    const id = exporting.taskId
    if (!id || exporting.running || exporting.status === 'idle' || handledExport.current === id) return
    handledExport.current = id
    Promise.resolve()
      .then(async () => {
        if (exporting.result?.content !== undefined) {
          await host.clipboard.writeText(exporting.result.content)
          toast.success(t('files.copied'))
        } else if (exporting.result?.saved) toast.success(t('files.saved'))
      })
      .catch((error) => toast.error(errorRef.current(error)))
      .finally(() => {
        pending.current = false
        if (mounted.current) setBusy(false)
      })
  }, [exporting.taskId, exporting.running, exporting.status, exporting.result, t])
  const sections: Section[] = ['conditions', 'timing', 'vary', 'directives', 'diagnostics', 'headers']
  const cancel = () => {
    const task = analysis.running ? analysis : query.running ? query : exporting
    void task.cancel().catch((error) => toast.error(errorMessage(error)))
  }
  return (
    <div className="@container flex h-full min-h-0 flex-col gap-3">
      <div className="flex shrink-0 items-center justify-between gap-3 rounded-card border border-border bg-surface p-3">
        <p className="min-w-0 text-xs leading-relaxed text-fg-muted">{t('boundary')}</p>
        <div className="shrink-0 @min-[56rem]:hidden">
          <SegmentedControl
            size="sm"
            value={pane}
            onValueChange={setPane}
            aria-label={t('panes.label')}
            options={[
              { value: 'input', label: t('panes.input') },
              { value: 'result', label: t('panes.result') },
            ]}
          />
        </div>
      </div>
      <div className="grid min-h-0 flex-1 grid-cols-1 gap-3 @min-[56rem]:grid-cols-[minmax(350px,0.9fr)_minmax(0,1.1fr)]">
        <div className={`${pane === 'input' ? 'flex' : 'hidden'} min-h-0 flex-col @min-[56rem]:flex`}>
          {cases ? (
            <InputForm
              cases={cases}
              selected={inputCase}
              compare={compare}
              busy={busy}
              select={setInputCase}
              change={(scenario) => {
                setCases(cases.map((item, i) => (i === inputCase ? scenario : item)))
                setDirty(true)
              }}
              copy={() => {
                setCases([cases[0], { ...cases[0], name: t('input.experiment') }])
                setInputCase(1)
                setDirty(true)
              }}
              run={() => {
                void inspect()
              }}
              setCompare={(value) => {
                setCompare(value)
                setDirty(true)
              }}
            />
          ) : (
            <div className="flex h-full items-center justify-center">
              {loadError ? (
                <p role="alert" className="text-xs text-danger">
                  {loadError}
                </p>
              ) : (
                <Spinner />
              )}
            </div>
          )}
        </div>
        <div className={`${pane === 'result' ? 'flex' : 'hidden'} min-h-0 flex-col @min-[56rem]:flex`}>
          <Panel
            className="min-h-0 flex-1"
            title={t('result.title')}
            icon={<Gauge />}
            bodyClassName="flex h-full min-h-0 flex-col gap-3 p-3"
            actions={
              <>
                <Tooltip content={t('files.copy')}>
                  <Button
                    size="icon-sm"
                    variant="ghost"
                    disabled={busy || !inspection}
                    aria-label={t('files.copy')}
                    onClick={() => void save('markdown', false)}
                  >
                    <Copy />
                  </Button>
                </Tooltip>
                <Tooltip content={t('files.markdown')}>
                  <Button
                    size="icon-sm"
                    variant="ghost"
                    disabled={busy || !inspection}
                    aria-label={t('files.markdown')}
                    onClick={() => void save('markdown', true)}
                  >
                    <Download />
                  </Button>
                </Tooltip>
                <Tooltip content={t('files.json')}>
                  <Button size="icon-sm" variant="ghost" disabled={busy || !inspection} aria-label={t('files.json')} onClick={() => void save('json', true)}>
                    <FileJson />
                  </Button>
                </Tooltip>
              </>
            }
          >
            {dirty && inspection && (
              <p role="status" className="shrink-0 text-xs text-warning">
                {t('result.dirty')}
              </p>
            )}
            {[analysis.error, query.error, exporting.error].map(
              (error, i) =>
                error && (
                  <p key={i} role="alert" className="shrink-0 text-xs text-danger">
                    {errorMessage(error)}
                  </p>
                ),
            )}
            {inspection && (
              <Cards
                summaries={inspection.summaries}
                selected={view?.case ?? 0}
                busy={busy}
                select={(index) => {
                  void request(index, section)
                }}
              />
            )}
            {view ? (
              <>
                <Select
                  className="w-full shrink-0"
                  size="sm"
                  disabled={busy}
                  value={section}
                  onValueChange={(value) => {
                    void request(view.case, value)
                  }}
                  aria-label={t('result.sections')}
                  options={sections.map((key) => ({ value: key, label: t(`sections.${key}`) }))}
                />
                <Detail
                  view={view}
                  section={section}
                  busy={busy}
                  page={(offset) => {
                    void request(view.case, section, offset)
                  }}
                />
              </>
            ) : (
              <div className="flex min-h-0 flex-1 items-center justify-center">
                <Empty icon={<Gauge />} title={t('result.empty')} description={t('result.emptyHint')} />
              </div>
            )}
            {busy && (
              <div className="flex shrink-0 items-center gap-2 text-xs text-fg-muted">
                <Spinner className="size-3.5" />
                <span>{t('result.working')}</span>
                <Button size="sm" variant="ghost" onClick={cancel}>
                  <Square />
                  {t('common.cancel')}
                </Button>
              </div>
            )}
            {(analysis.logs.length > 0 || query.logs.length > 0) && (
              <details className="shrink-0 rounded-control border border-border">
                <summary className="cursor-pointer px-2 py-1 text-[11px] text-fg-subtle">{t('logs.title')}</summary>
                <LogViewer entries={analysis.logs} className="h-20" emptyText={t('logs.empty')} jumpLabel={t('logs.latest')} />
              </details>
            )}
          </Panel>
        </div>
      </div>
    </div>
  )
}
