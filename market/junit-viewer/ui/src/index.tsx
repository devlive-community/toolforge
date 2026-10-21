import { useEffect, useRef, useState } from 'react'
import { host, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { Badge, Button, Empty, Input, LogViewer, Panel, Progress, SegmentedControl, Select, Tooltip, cn, toast } from '@toolforge/ui'
import { ArrowLeft, Download, Files, FileCode2, FolderOpen, FolderTree, ListChecks, Plus, Square, TestTube2, TriangleAlert } from 'lucide-react'
import { Busy, Cases, Details, Diagnostics, Metrics, Sources, Suites } from './Browser'
import type { Case, Filter, Source, Status, Suite, Summary } from './types'

export default function JunitViewer() {
  const { t, errorMessage } = usePlugin()
  const task = useTask<Summary>()
  const exporting = useTask<{ saved: boolean }>()
  const [paths, setPaths] = useState<string[]>([])
  const [source, setSource] = useState<Source | null>(null)
  const [trail, setTrail] = useState<Suite[]>([])
  const [selected, setSelected] = useState<Case | null>(null)
  const [sourcePage, setSourcePage] = useState(1)
  const [suitePage, setSuitePage] = useState(1)
  const [page, setPage] = useState(1)
  const [diagnosticPage, setDiagnosticPage] = useState(1)
  const [query, setQuery] = useState('')
  const [status, setStatus] = useState<Status | 'all'>('all')
  const [slowest, setSlowest] = useState(false)
  const [diagnostics, setDiagnostics] = useState(false)
  const [smallView, setSmallView] = useState<'cases' | 'detail'>('cases')
  const busy = task.running || exporting.running
  const report = task.result
  const suite = trail.at(-1)
  const filter: Filter = { source: source?.id, suite: suite?.id, status: status === 'all' ? undefined : status, query, slowest }
  const resetView = () => {
    setSource(null)
    setTrail([])
    setSelected(null)
    setSourcePage(1)
    setSuitePage(1)
    setPage(1)
    setDiagnosticPage(1)
    setDiagnostics(false)
    setQuery('')
    setStatus('all')
    setSmallView('cases')
  }
  const load = (picked: string[], append: boolean) => {
    if (picked.length === 0 || busy) return
    const next = append ? [...paths, ...picked] : picked
    setPaths(next)
    resetView()
    exporting.reset()
    void task.start('load', { paths: next })
  }
  const pick = async (append: boolean) => {
    try {
      load(await host.dialog.openFiles([{ name: t('open.filter'), extensions: ['xml'] }]), append)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const loadRef = useRef(load)
  useEffect(() => {
    loadRef.current = load
  })
  useEffect(() => {
    const off = host.onFileDrop({ drop: (picked) => loadRef.current(picked, false) })
    return () => {
      void off.then((unlisten) => unlisten())
    }
  }, [])
  const save = async (format: 'csv' | 'json' | 'markdown') => {
    if (!report || busy) return
    try {
      const ext = format === 'markdown' ? 'md' : format
      const path = await host.dialog.saveFile(`junit-report.${ext}`, [{ name: t(`export.${format}`), extensions: [ext] }])
      if (path) await exporting.start('export', { handle: report.handle, filter, format, path })
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const cancel = async () => {
    try {
      await (task.running ? task.cancel() : exporting.cancel())
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const changeSource = (next: Source | null) => {
    setSource(next)
    setTrail([])
    setSelected(null)
    setSuitePage(1)
    setPage(1)
    setSmallView('cases')
  }
  const changeSuite = (next: Suite) => {
    setTrail([...trail, next])
    setSelected(null)
    setSuitePage(1)
    setPage(1)
    setSmallView('cases')
  }
  const back = () => {
    setTrail(trail.slice(0, -1))
    setSelected(null)
    setSuitePage(1)
    setPage(1)
    setSmallView('cases')
  }

  return (
    <div className="@container flex h-full min-h-0 flex-col gap-3">
      <div className="flex shrink-0 flex-wrap items-center gap-2">
        <Button size="sm" disabled={busy} onClick={() => void pick(false)}>
          <FolderOpen />
          {t('open.title')}
        </Button>
        {report && (
          <Button size="sm" disabled={busy} onClick={() => void pick(true)}>
            <Plus />
            {t('open.add')}
          </Button>
        )}
        {busy && (
          <Button size="sm" onClick={() => void cancel()}>
            <Square />
            {t('cancel')}
          </Button>
        )}
        <span className="min-w-0 flex-1 truncate text-[11px] text-fg-subtle">{t('privacy')}</span>
        {report && (
          <>
            {(['csv', 'json', 'markdown'] as const).map((format) => (
              <Tooltip key={format} content={t('export.hint')}>
                <Button size="sm" disabled={busy} onClick={() => void save(format)}>
                  <Download />
                  {t(`export.${format}`)}
                </Button>
              </Tooltip>
            ))}
          </>
        )}
      </div>
      <Busy error={task.error ?? exporting.error} running={busy} />
      {exporting.status === 'succeeded' && (
        <p role="status" className="text-xs text-success">
          {t('export.saved')}
        </p>
      )}
      {task.progress && task.running && <Progress value={(task.progress.done / Math.max(task.progress.total, 1)) * 100} aria-label={t('loading')} />}
      {report ? (
        <>
          <Metrics report={report} />
          <div className="flex shrink-0 flex-wrap items-center gap-2 text-[11px] text-fg-muted">
            <Badge>{t('summary.files', { count: report.sourceCount })}</Badge>
            <span>{t('summary.time', { known: report.counts.knownSeconds, missing: report.counts.missingTime })}</span>
            <span className="flex-1" />
            <Button size="sm" variant={diagnostics ? 'secondary' : 'ghost'} onClick={() => setDiagnostics(!diagnostics)}>
              <TriangleAlert />
              {t('diagnostics.count', { count: report.diagnosticCount })}
            </Button>
          </div>
          {diagnostics ? (
            <div className="min-h-0 flex-1">
              <Diagnostics key={`${report.handle}:${diagnosticPage}`} handle={report.handle} page={diagnosticPage} setPage={setDiagnosticPage} />
            </div>
          ) : (
            <div className="grid min-h-0 flex-1 grid-cols-[200px_minmax(0,1fr)] @min-[1000px]:grid-cols-[230px_minmax(0,1fr)] gap-3">
              <div className="grid min-h-0 grid-rows-2 gap-3">
                <Panel
                  title={t('source.title')}
                  icon={<Files />}
                  bodyClassName="flex h-full min-h-0 flex-col gap-2 p-3"
                  actions={
                    <Button size="sm" variant="ghost" onClick={() => changeSource(null)}>
                      {t('source.all')}
                    </Button>
                  }
                >
                  <Sources
                    key={`${report.handle}:${sourcePage}`}
                    handle={report.handle}
                    page={sourcePage}
                    selected={source?.id}
                    onSelect={changeSource}
                    setPage={setSourcePage}
                  />
                </Panel>
                <Panel
                  title={suite?.name || t('suite.title')}
                  icon={<FolderTree />}
                  bodyClassName="flex h-full min-h-0 flex-col gap-2 p-3"
                  actions={
                    suite && (
                      <Tooltip content={t('suite.back')}>
                        <Button size="icon-sm" variant="ghost" aria-label={t('suite.back')} onClick={back}>
                          <ArrowLeft />
                        </Button>
                      </Tooltip>
                    )
                  }
                >
                  {suite && <p className="shrink-0 text-[11px] text-fg-subtle">{t('suite.selected', { id: suite.id })}</p>}
                  <Suites
                    key={`${report.handle}:${source?.id}:${suite?.id}:${suitePage}`}
                    handle={report.handle}
                    source={source?.id}
                    parent={suite?.id}
                    page={suitePage}
                    setPage={setSuitePage}
                    onSelect={changeSuite}
                  />
                </Panel>
              </div>
              <div className="flex min-h-0 flex-col gap-3 @min-[1000px]:grid @min-[1000px]:grid-cols-[minmax(290px,1fr)_minmax(300px,1.1fr)]">
                <div className="shrink-0 @min-[1000px]:hidden">
                  <SegmentedControl
                    size="sm"
                    value={smallView}
                    onValueChange={setSmallView}
                    aria-label={t('detail.view')}
                    options={[
                      { value: 'cases', label: t('cases.title') },
                      { value: 'detail', label: t('detail.title') },
                    ]}
                  />
                </div>
                <Panel
                  className={cn('min-h-0 flex-1', smallView === 'detail' && 'hidden @min-[1000px]:flex')}
                  title={t('cases.title')}
                  icon={<ListChecks />}
                  bodyClassName="flex h-full min-h-0 flex-col gap-3 p-3"
                >
                  <Input
                    size="sm"
                    value={query}
                    maxLength={512}
                    onChange={(event) => {
                      setQuery(event.target.value)
                      setPage(1)
                      setSelected(null)
                    }}
                    placeholder={t('cases.search')}
                    aria-label={t('cases.search')}
                  />
                  <div className="flex shrink-0 flex-wrap gap-2">
                    <Select
                      size="sm"
                      value={status}
                      onValueChange={(value) => {
                        setStatus(value)
                        setPage(1)
                        setSelected(null)
                      }}
                      options={(['all', 'passed', 'failure', 'error', 'skipped', 'unknown'] as const).map((value) => ({ value, label: t(`status.${value}`) }))}
                      aria-label={t('cases.status')}
                    />
                    <Select
                      size="sm"
                      value={slowest ? 'slow' : 'original'}
                      onValueChange={(value) => {
                        setSlowest(value === 'slow')
                        setPage(1)
                      }}
                      options={[
                        { value: 'original', label: t('cases.original') },
                        { value: 'slow', label: t('cases.slow') },
                      ]}
                      aria-label={t('cases.sort')}
                    />
                  </div>
                  <Cases
                    key={`${report.handle}:${source?.id}:${suite?.id}:${status}:${query}:${slowest}:${page}`}
                    handle={report.handle}
                    filter={filter}
                    page={page}
                    setPage={setPage}
                    selected={selected}
                    onSelect={(value) => {
                      setSelected(value)
                      setSmallView('detail')
                    }}
                  />
                </Panel>
                <Panel
                  className={cn('min-h-0 flex-1', smallView === 'cases' && 'hidden @min-[1000px]:flex')}
                  title={t('detail.title')}
                  icon={<FileCode2 />}
                  bodyClassName="flex h-full min-h-0 flex-col p-3"
                >
                  {selected ? (
                    <Details key={`case:${selected.id}`} handle={report.handle} kind="case" id={selected.id} />
                  ) : suite ? (
                    <Details key={`suite:${suite.id}`} handle={report.handle} kind="suite" id={suite.id} />
                  ) : (
                    <Empty icon={<TestTube2 />} title={t('detail.empty')} description={t('detail.hint')} />
                  )}
                </Panel>
              </div>
            </div>
          )}
        </>
      ) : (
        !task.running && (
          <Panel bodyClassName="flex h-full items-center justify-center p-3" className="min-h-0 flex-1">
            <div className="max-w-md space-y-4 text-center">
              <Empty icon={<TestTube2 />} title={t('empty.title')} description={t('empty.hint')} />
              <Button variant="primary" onClick={() => void pick(false)}>
                <FolderOpen />
                {t('open.title')}
              </Button>
              <p className="text-xs text-fg-subtle">{t('limits')}</p>
            </div>
          </Panel>
        )
      )}
      {(task.logs.length > 0 || exporting.logs.length > 0) && (
        <details className="shrink-0 rounded-control border border-border">
          <summary className="cursor-pointer p-2 text-xs text-fg-muted">{t('logs.title')}</summary>
          <LogViewer entries={exporting.running ? exporting.logs : task.logs} className="h-24" emptyText={t('logs.empty')} jumpLabel={t('logs.latest')} />
        </details>
      )}
    </div>
  )
}
