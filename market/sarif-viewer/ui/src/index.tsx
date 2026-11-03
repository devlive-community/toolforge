import { useEffect, useRef, useState } from 'react'
import { host, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { Badge, Button, Empty, Input, LogViewer, Panel, Progress, SegmentedControl, Select, Tooltip, cn, toast } from '@toolforge/ui'
import { Download, FileSearch, FolderOpen, ListChecks, Plus, ShieldQuestion, Square, TriangleAlert } from 'lucide-react'
import { Artifacts, Busy, CountsStrip, Diagnostics, Results, ResultDetail, Rules, Runs } from './Browser'
import type { ArtifactRow, Filter, ResultRow, RuleRow, RunRow, Summary } from './types'

type ListTab = 'runs' | 'rules' | 'artifacts'
type SuppressedChoice = 'all' | 'only' | 'excluded'

export default function SarifViewer() {
  const { t, errorMessage } = usePlugin()
  const task = useTask<Summary>()
  const exporting = useTask<{ saved: boolean }>()
  const [paths, setPaths] = useState<string[]>([])
  const [run, setRun] = useState<RunRow | null>(null)
  const [rule, setRule] = useState<RuleRow | null>(null)
  const [artifact, setArtifact] = useState<ArtifactRow | null>(null)
  const [selected, setSelected] = useState<ResultRow | null>(null)
  const [page, setPage] = useState(1)
  const [diagnosticPage, setDiagnosticPage] = useState(1)
  const [query, setQuery] = useState('')
  const [level, setLevel] = useState<'all' | 'error' | 'warning' | 'note' | 'none'>('all')
  const [suppressed, setSuppressed] = useState<SuppressedChoice>('all')
  const [listTab, setListTab] = useState<ListTab>('runs')
  const [smallView, setSmallView] = useState<'results' | 'detail'>('results')
  const [diagnosticsOn, setDiagnosticsOn] = useState(false)
  const busy = task.running || exporting.running
  const report = task.result
  const filter: Filter = {
    run: run?.id,
    ruleId: rule?.ruleId,
    artifact: artifact?.id,
    level: level === 'all' ? undefined : level,
    suppressed: suppressed === 'all' ? undefined : suppressed === 'only',
    query,
  }
  const resetView = () => {
    setRun(null)
    setRule(null)
    setArtifact(null)
    setSelected(null)
    setPage(1)
    setDiagnosticPage(1)
    setDiagnosticsOn(false)
    setQuery('')
    setLevel('all')
    setSuppressed('all')
    setListTab('runs')
    setSmallView('results')
  }
  const load = (picked: string[], append: boolean) => {
    if (picked.length === 0 || busy) return
    const next = append ? [...paths, ...picked] : picked
    setPaths(next)
    resetView()
    exporting.reset()
    void task.start('parse', { paths: next })
  }
  const pick = async (append: boolean) => {
    try {
      load(await host.dialog.openFiles([{ name: t('open.filter'), extensions: ['sarif', 'json'] }]), append)
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
      const path = await host.dialog.saveFile(`sarif-report.${ext}`, [{ name: t(`export.${format}`), extensions: [ext] }])
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
  const changeRun = (next: RunRow | null) => {
    setRun(next)
    setRule(null)
    setArtifact(null)
    setSelected(null)
    setPage(1)
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
          <CountsStrip counts={report.counts} />
          <div className="flex shrink-0 flex-wrap items-center gap-2 text-[11px] text-fg-muted">
            <Badge>{t('summary.runs', { count: report.runCount })}</Badge>
            <Badge>{t('summary.files', { count: report.fileCount })}</Badge>
            <span>{t('summary.tables', { rules: report.ruleCount, artifacts: report.artifactCount })}</span>
            <span className="flex-1" />
            <Button size="sm" variant={diagnosticsOn ? 'secondary' : 'ghost'} onClick={() => setDiagnosticsOn(!diagnosticsOn)}>
              <TriangleAlert />
              {t('diagnostics.count', { count: report.diagnosticCount })}
            </Button>
          </div>
          {diagnosticsOn ? (
            <div className="min-h-0 flex-1">
              <Diagnostics key={`${report.handle}:${diagnosticPage}`} handle={report.handle} run={run?.id} page={diagnosticPage} setPage={setDiagnosticPage} />
            </div>
          ) : (
            <div className="grid min-h-0 flex-1 grid-cols-[210px_minmax(0,1fr)] @min-[1000px]:grid-cols-[230px_minmax(0,1fr)] gap-3">
              <Panel
                title={t(`${listTab}.title`)}
                bodyClassName="flex h-full min-h-0 flex-col gap-2 p-3"
                extra={
                  <SegmentedControl
                    size="sm"
                    value={listTab}
                    onValueChange={setListTab}
                    aria-label={t('list.tab')}
                    options={[
                      { value: 'runs', label: t('runs.title') },
                      { value: 'rules', label: t('rules.title') },
                      { value: 'artifacts', label: t('artifacts.title') },
                    ]}
                  />
                }
              >
                {listTab === 'runs' && <Runs key={`${report.handle}:runs`} handle={report.handle} selected={run?.id} onSelect={changeRun} />}
                {listTab === 'rules' && <Rules key={`${report.handle}:rules:${run?.id ?? 'all'}`} handle={report.handle} run={run?.id} selected={rule?.ruleId} onSelect={setRule} />}
                {listTab === 'artifacts' && <Artifacts key={`${report.handle}:artifacts:${run?.id ?? 'all'}`} handle={report.handle} run={run?.id} selected={artifact?.id} onSelect={setArtifact} />}
              </Panel>
              <div className="flex min-h-0 flex-col gap-3 @min-[1000px]:grid @min-[1000px]:grid-cols-[minmax(300px,1fr)_minmax(320px,1.1fr)]">
                <div className="shrink-0 @min-[1000px]:hidden">
                  <SegmentedControl
                    size="sm"
                    value={smallView}
                    onValueChange={setSmallView}
                    aria-label={t('detail.view')}
                    options={[
                      { value: 'results', label: t('results.title') },
                      { value: 'detail', label: t('detail.title') },
                    ]}
                  />
                </div>
                <Panel
                  className={cn('min-h-0 flex-1', smallView === 'detail' && 'hidden @min-[1000px]:flex')}
                  title={t('results.title')}
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
                    placeholder={t('results.search')}
                    aria-label={t('results.search')}
                  />
                  <div className="flex shrink-0 flex-wrap gap-2">
                    <Select
                      size="sm"
                      value={level}
                      onValueChange={(value) => {
                        setLevel(value)
                        setPage(1)
                        setSelected(null)
                      }}
                      options={(['all', 'error', 'warning', 'note', 'none'] as const).map((value) => ({ value, label: value === 'all' ? t('level.all') : t(`level.${value}`) }))}
                      aria-label={t('results.level')}
                    />
                    <Select
                      size="sm"
                      value={suppressed}
                      onValueChange={(value) => {
                        setSuppressed(value)
                        setPage(1)
                        setSelected(null)
                      }}
                      options={(['all', 'only', 'excluded'] as const).map((value) => ({ value, label: t(`suppressed.${value}`) }))}
                      aria-label={t('results.suppressed')}
                    />
                  </div>
                  <Results
                    key={`${report.handle}:${JSON.stringify(filter)}:${page}`}
                    handle={report.handle}
                    filter={filter}
                    page={page}
                    selected={selected}
                    onSelect={(row) => {
                      setSelected(row)
                      setSmallView('detail')
                    }}
                    setPage={setPage}
                  />
                </Panel>
                <Panel
                  className={cn('min-h-0 flex-1', smallView === 'results' && 'hidden @min-[1000px]:flex')}
                  title={t('detail.title')}
                  icon={<FileSearch />}
                  bodyClassName="flex h-full min-h-0 flex-col p-3"
                >
                  {selected ? (
                    <ResultDetail key={`${report.handle}:result:${selected.id}`} handle={report.handle} id={selected.id} />
                  ) : (
                    <Empty icon={<ShieldQuestion />} title={t('detail.empty')} description={t('detail.hint')} />
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
              <Empty icon={<ShieldQuestion />} title={t('empty.title')} description={t('empty.hint')} />
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
