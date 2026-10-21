import { useEffect, useRef, useState } from 'react'
import { host, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { Badge, Button, Empty, Input, LogViewer, Panel, SegmentedControl, toast } from '@toolforge/ui'
import { Boxes, Copy, Download, Files, FolderOpen, TriangleAlert } from 'lucide-react'
import { Analyze, OutputPage } from './Analyze'
import { Compare } from './Compare'
import { Busy, Bytes, Delta, Pager, PAGE, useRequest } from './Widgets'
import type { Change, Diagnostic, Page, Pair, Query, Side, Summary } from './types'

function Diagnostics({ q, page, setPage }: { q: Query; page: number; setPage: (n: number) => void }) {
  const { t } = usePlugin()
  const task = useRequest<Page<Diagnostic>>('query', { ...q, kind: 'diagnostics', offset: (page - 1) * PAGE, limit: PAGE })
  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <Busy task={task} />
      <div className="min-h-0 flex-1 space-y-2 overflow-auto">
        {task.result?.items.map((d, i) => (
          <div key={i} className="space-y-1 rounded-control border border-border bg-warning-soft p-3">
            <p className="text-xs text-warning">{t(`diagnostics.${d.kind}`, { count: d.count })}</p>
            <code className="block break-words text-[11px] text-fg-muted">{d.path}</code>
          </div>
        ))}
        {task.result?.total === 0 && <p className="text-xs text-fg-muted">{t('diagnostics.none')}</p>}
      </div>
      <Pager page={page} total={task.result?.total ?? 0} setPage={setPage} />
    </div>
  )
}

export default function BundleAnalyzer() {
  const { t, errorMessage } = usePlugin()
  const loading = useTask<Summary>()
  const exporting = useTask<{ saved?: boolean; content?: string }>()
  const [report, setReport] = useState<Summary | null>(null)
  const [paths, setPaths] = useState<{ before: string; after?: string } | null>(null)
  const pendingPaths = useRef<{ before: string; after?: string } | null>(null)
  const handledExport = useRef<string | null>(null)
  const [side, setSide] = useState<Side>('before')
  const [output, setOutput] = useState<string | null>(null)
  const [directory, setDirectory] = useState<string[]>([])
  const [outputSearch, setOutputSearch] = useState('')
  const [outputPage, setOutputPage] = useState(1)
  const [moduleSearch, setModuleSearch] = useState('')
  const [mode, setMode] = useState<'analyze' | 'compare' | 'diagnostics'>('analyze')
  const [pairs, setPairs] = useState<Pair[]>([])
  const [change, setChange] = useState<Change | null>(null)
  const [changeSearch, setChangeSearch] = useState('')
  const [diagnosticPage, setDiagnosticPage] = useState(1)
  const [dialog, setDialog] = useState(false)
  const busy = loading.running || exporting.running || dialog
  useEffect(() => {
    if (!loading.result) return
    const next = loading.result
    void Promise.resolve().then(() => {
      setReport(next)
      setPaths(pendingPaths.current)
      setSide('before')
      setOutput(next.before.firstOutput)
      setDirectory([])
      setOutputPage(1)
      setOutputSearch('')
      setModuleSearch('')
      setPairs([])
      setChange(null)
      setMode('analyze')
      setDiagnosticPage(1)
    })
  }, [loading.result])
  useEffect(() => {
    if (!exporting.result?.content || handledExport.current === exporting.taskId) return
    handledExport.current = exporting.taskId
    void host.clipboard
      .writeText(exporting.result.content)
      .then(() => toast.success(t('export.copied')))
      .catch((e) => toast.error(errorMessage(e)))
  }, [exporting.result, exporting.taskId, t, errorMessage])
  const pick = async (after: boolean) => {
    if (busy) return
    setDialog(true)
    try {
      const path = await host.dialog.openFile([{ name: t('open.filter'), extensions: ['json'] }])
      if (!path) return
      const next: { before: string; after?: string } = after && paths ? { ...paths, after: path } : { before: path }
      pendingPaths.current = next
      await loading.start('load', { beforePath: next.before, afterPath: next.after })
    } catch (e) {
      toast.error(errorMessage(e))
    } finally {
      setDialog(false)
    }
  }
  const chooseSide = (next: Side) => {
    setSide(next)
    setOutput(report?.[next]?.firstOutput ?? null)
    setDirectory([])
    setOutputPage(1)
    setOutputSearch('')
    setModuleSearch('')
    setDiagnosticPage(1)
  }
  const query: Query = {
    handle: report?.handle ?? '',
    side,
    kind: mode === 'compare' ? (change ? 'diffModules' : 'compare') : mode === 'diagnostics' ? 'diagnostics' : output ? 'modules' : 'outputs',
    output: mode === 'compare' ? change?.before : output,
    other: change?.after,
    pairs,
    search: mode === 'compare' ? changeSearch : moduleSearch,
    directory,
  }
  const save = async (format: 'csv' | 'json' | 'markdown', copy = false) => {
    if (!report || busy) return
    setDialog(true)
    try {
      const ext = format === 'markdown' ? 'md' : format
      const path = copy ? null : await host.dialog.saveFile(`bundle-report.${ext}`, [{ name: t(`export.${format}`), extensions: [ext] }])
      if (!copy && !path) return
      await exporting.start(copy ? 'copy' : 'export', { query, format, path })
    } catch (e) {
      toast.error(errorMessage(e))
    } finally {
      setDialog(false)
    }
  }
  const selected = report?.[side]
  return (
    <div className="@container flex h-full min-h-0 flex-col gap-3">
      <div className="flex shrink-0 flex-wrap items-center gap-2">
        <Button size="sm" disabled={busy} onClick={() => void pick(false)}>
          <FolderOpen />
          {t('open.before')}
        </Button>
        <Button size="sm" disabled={busy || !report} onClick={() => void pick(true)}>
          <Files />
          {t('open.after')}
        </Button>
        <span className="min-w-0 flex-1 truncate text-[10px] text-fg-subtle">{t('boundary')}</span>
        {report && (
          <>
            <Button size="sm" disabled={busy} onClick={() => void save('markdown', true)}>
              <Copy />
              {t('export.copy')}
            </Button>
            {(['csv', 'json', 'markdown'] as const).map((format) => (
              <Button size="sm" key={format} disabled={busy} onClick={() => void save(format)}>
                <Download />
                {t(`export.${format}`)}
              </Button>
            ))}
          </>
        )}
      </div>
      <Busy task={loading} />
      <Busy task={exporting} />
      {exporting.result?.saved && (
        <p role="status" className="text-xs text-success">
          {t('export.saved')}
        </p>
      )}
      {report && selected ? (
        <>
          <div className="flex shrink-0 flex-wrap items-center gap-3">
            <SegmentedControl
              size="sm"
              value={side}
              onValueChange={chooseSide}
              aria-label={t('side.label')}
              options={
                report.after
                  ? [
                      { value: 'before', label: t('side.before') },
                      { value: 'after', label: t('side.after') },
                    ]
                  : [{ value: 'before', label: t('side.before') }]
              }
            />
            <span className="min-w-0 flex-1 truncate text-xs text-fg-muted">{selected.name}</span>
            <Badge>{t('summary.counts', { inputs: selected.totals.inputs, outputs: selected.totals.outputs })}</Badge>
            <span className="text-xs text-fg">
              <Bytes value={selected.totals.bytes} />
            </span>
          </div>
          <div className="flex shrink-0 flex-wrap gap-2">
            <Button size="sm" variant={mode === 'analyze' ? 'secondary' : 'ghost'} onClick={() => setMode('analyze')}>
              <Boxes />
              {t('mode.analyze')}
            </Button>
            <Button size="sm" disabled={!report.after} variant={mode === 'compare' ? 'secondary' : 'ghost'} onClick={() => setMode('compare')}>
              {t('mode.compare')}
            </Button>
            <Button size="sm" variant={mode === 'diagnostics' ? 'secondary' : 'ghost'} onClick={() => setMode('diagnostics')}>
              <TriangleAlert />
              {t('diagnostics.count', { count: selected.diagnostics })}
            </Button>
          </div>
          <div
            className={
              busy
                ? 'pointer-events-none grid min-h-0 flex-1 grid-cols-[220px_minmax(0,1fr)] gap-3 opacity-60'
                : 'grid min-h-0 flex-1 grid-cols-[220px_minmax(0,1fr)] gap-3'
            }
          >
            <Panel title={t('output.title')} icon={<Files />} bodyClassName="flex h-full min-h-0 flex-col gap-3 p-3">
              <Input
                size="sm"
                value={outputSearch}
                maxLength={512}
                onChange={(e) => {
                  setOutputSearch(e.target.value)
                  setOutputPage(1)
                }}
                placeholder={t('output.search')}
                aria-label={t('output.search')}
              />
              <OutputPage
                key={`${report.handle}:${side}:${outputSearch}:${outputPage}`}
                q={{ handle: report.handle, kind: 'outputs', side, search: outputSearch }}
                page={outputPage}
                setPage={setOutputPage}
                selected={output}
                select={(path) => {
                  setOutput(path)
                  setDirectory([])
                  setModuleSearch('')
                  setMode('analyze')
                }}
              />
            </Panel>
            <Panel
              title={mode === 'compare' ? t('mode.compare') : mode === 'diagnostics' ? t('diagnostics.title') : (output ?? t('modules.title'))}
              bodyClassName="flex h-full min-h-0 flex-col gap-3 p-3"
            >
              {mode === 'compare' ? (
                <>
                  <p className="shrink-0 text-xs text-fg-muted">
                    {t('compare.total')} <Bytes value={report.before.totals.bytes} /> {t('compare.to')} <Bytes value={report.after?.totals.bytes} />{' '}
                    {report.totalDelta != null && <Delta value={report.totalDelta} />}
                  </p>
                  <Compare q={query} pairs={pairs} setPairs={setPairs} selected={change} select={setChange} search={changeSearch} setSearch={setChangeSearch} />
                </>
              ) : mode === 'diagnostics' ? (
                <Diagnostics key={`${report.handle}:${side}:${diagnosticPage}`} q={query} page={diagnosticPage} setPage={setDiagnosticPage} />
              ) : output ? (
                <Analyze
                  key={`${report.handle}:${side}:${output}`}
                  q={{ handle: report.handle, kind: 'modules', side, output }}
                  directory={directory}
                  navigate={setDirectory}
                  search={moduleSearch}
                  setSearch={setModuleSearch}
                />
              ) : (
                <Empty icon={<Boxes />} title={t('output.empty')} />
              )}
            </Panel>
          </div>
        </>
      ) : (
        <Panel className="min-h-0 flex-1" bodyClassName="flex h-full items-center justify-center p-3">
          <Empty icon={<Boxes />} title={t('empty.title')} description={t('empty.hint')} />
        </Panel>
      )}
      {loading.logs.length > 0 && (
        <details className="shrink-0 rounded-control border border-border">
          <summary className="cursor-pointer p-2 text-[10px] text-fg-muted">{t('logs.title')}</summary>
          <LogViewer entries={loading.logs} className="h-20" emptyText={t('logs.empty')} jumpLabel={t('logs.latest')} />
        </details>
      )}
    </div>
  )
}
