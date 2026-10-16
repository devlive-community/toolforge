import { useEffect, useMemo, useRef, useState } from 'react'
import { Badge, Button, DropdownMenu, Empty, Input, Modal, Panel, Progress, SegmentedControl, Select, Spinner, Tooltip, cn, toast } from '@toolforge/ui'
import { host, usePlugin, usePluginState, useTask } from '@toolforge/plugin-ui-sdk'
import { ArrowRight, ChevronDown, ChevronLeft, ChevronRight, Download, FileCode2, FolderOpen, FolderTree, Plus, Settings2, ShieldAlert, Trash2, Upload } from 'lucide-react'
import { ratio, type Counts, type Detail, type Entry, type Mapping, type Metrics, type Overview } from './types'

const PAGE = 200
type Kind = 'files' | 'dirs'
type Sort = 'path' | 'lines' | 'functions' | 'branches'

function pctText(counts: Counts) {
  const value = ratio(counts)
  return value === null ? 'N/A' : `${value.toFixed(1)}%`
}

function tone(counts: Counts) {
  const value = ratio(counts)
  if (value === null) return 'text-fg-subtle'
  if (value >= 80) return 'text-success'
  if (value >= 50) return 'text-warning'
  return 'text-danger'
}

function Bar({ counts }: { counts: Counts }) {
  const value = ratio(counts)
  return (
    <div className="h-1 w-full rounded-full bg-surface-2">
      {value !== null && <div className={cn('h-1 rounded-full', value >= 80 ? 'bg-success' : value >= 50 ? 'bg-warning' : 'bg-danger')} style={{ width: `${Math.max(value, 1)}%` }} />}
    </div>
  )
}

function MetricCard({ label, counts, extra }: { label: string; counts: Counts; extra?: string }) {
  return (
    <div className="min-w-0 rounded-control border border-border px-3 py-2">
      <p className="text-[11px] text-fg-muted">{label}</p>
      <p className={cn('text-lg font-semibold tabular-nums', tone(counts))}>{pctText(counts)}</p>
      <p className="text-[11px] text-fg-subtle tabular-nums">
        {counts.hit} / {counts.found}
        {extra && ` · ${extra}`}
      </p>
    </div>
  )
}

function Metrics3({ metrics }: { metrics: Metrics }) {
  const { t } = usePlugin()
  return (
    <div className="grid grid-cols-3 gap-2">
      <MetricCard label={t('metric.lines')} counts={metrics.lines} />
      <MetricCard label={t('metric.functions')} counts={metrics.functions} />
      <MetricCard
        label={t('metric.branches')}
        counts={metrics.branches}
        extra={metrics.branchesNotExecuted > 0 ? t('metric.notExecuted', { count: metrics.branchesNotExecuted }) : undefined}
      />
    </div>
  )
}

function Settings({ open, onClose, root, setRoot, mappings, setMappings }: { open: boolean; onClose: () => void; root: string; setRoot: (v: string) => void; mappings: Mapping[]; setMappings: (v: Mapping[]) => void }) {
  const { t, errorMessage } = usePlugin()
  const pickRoot = async () => {
    try {
      const dir = await host.dialog.openDirectory()
      if (dir) setRoot(dir)
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }
  return (
    <Modal open={open} onOpenChange={(next) => !next && onClose()} title={t('source.title')} description={t('source.description')} closeLabel={t('source.close')}>
      <div className="space-y-3">
        <div className="space-y-1">
          <p className="text-xs text-fg-muted">{t('source.root')}</p>
          <div className="flex gap-2">
            <Input size="sm" className="flex-1 font-mono" value={root} onChange={(event) => setRoot(event.target.value)} placeholder={t('source.rootPlaceholder')} aria-label={t('source.root')} />
            <Button size="sm" onClick={() => void pickRoot()}>
              <FolderOpen />
              {t('source.choose')}
            </Button>
          </div>
        </div>
        <div className="space-y-1.5">
          <p className="text-xs text-fg-muted">{t('source.mappings')}</p>
          {mappings.map((mapping, index) => (
            <div key={index} className="flex items-center gap-1.5">
              <Input
                size="sm"
                className="flex-1 font-mono"
                value={mapping.from}
                placeholder={t('source.from')}
                aria-label={t('source.from')}
                onChange={(event) => setMappings(mappings.map((m, i) => (i === index ? { ...m, from: event.target.value } : m)))}
              />
              <ArrowRight className="size-3.5 shrink-0 text-fg-subtle" aria-hidden />
              <Input
                size="sm"
                className="flex-1 font-mono"
                value={mapping.to}
                placeholder={t('source.to')}
                aria-label={t('source.to')}
                onChange={(event) => setMappings(mappings.map((m, i) => (i === index ? { ...m, to: event.target.value } : m)))}
              />
              <Button size="icon-sm" variant="ghost" aria-label={t('source.removeMapping')} onClick={() => setMappings(mappings.filter((_, i) => i !== index))}>
                <Trash2 />
              </Button>
            </div>
          ))}
          <Button size="sm" variant="ghost" onClick={() => setMappings([...mappings, { from: '', to: '' }])}>
            <Plus />
            {t('source.addMapping')}
          </Button>
          <p className="text-[11px] leading-snug text-fg-subtle">{t('source.mappingHint')}</p>
        </div>
      </div>
    </Modal>
  )
}

function FileDetail({ detail, sourceNote }: { detail: Detail; sourceNote: string | null }) {
  const { t } = usePlugin()
  const lineRefs = useRef(new Map<number, HTMLDivElement>())
  const jump = (line: number) => lineRefs.current.get(line)?.scrollIntoView({ block: 'center' })
  const hasText = detail.lines.some((line) => line.text !== undefined)
  return (
    <div className="space-y-3">
      <Metrics3 metrics={detail.metrics} />
      {sourceNote && <p className="rounded-control bg-surface-2 px-3 py-2 text-xs text-fg-muted">{sourceNote}</p>}
      {detail.sourceIssueCount > 0 && (
        <p className="text-xs text-warning">
          {t('detail.sourceIssues', { count: detail.sourceIssueCount })}{' '}
          {detail.sourceIssues.slice(0, 8).map((issue) => (
            <button key={`${issue.kind}-${issue.line}`} type="button" className="mr-1 underline" onClick={() => jump(issue.line)}>
              {t(`detail.${issue.kind}`, { line: issue.line })}
            </button>
          ))}
        </p>
      )}
      {detail.uncovered.length > 0 && (
        <div className="flex flex-wrap items-center gap-1 text-xs">
          <span className="text-fg-muted">{t('detail.uncovered')}</span>
          {detail.uncovered.slice(0, 40).map((range) => (
            <button key={range.start} type="button" onClick={() => jump(range.start)} className="rounded bg-danger-soft px-1.5 py-0.5 font-mono text-danger tabular-nums">
              {range.start === range.end ? range.start : `${range.start}–${range.end}`}
            </button>
          ))}
          {detail.uncovered.length > 40 && <span className="text-fg-subtle">{t('detail.more', { count: detail.uncovered.length - 40 })}</span>}
        </div>
      )}
      <div className="overflow-auto rounded-control border border-border font-mono text-xs">
        {detail.lines.map((line) => (
          <div
            key={line.line}
            ref={(node) => {
              if (node) lineRefs.current.set(line.line, node)
              else lineRefs.current.delete(line.line)
            }}
            className={cn('flex min-w-max', line.hits === 0 ? 'bg-danger-soft' : line.hits !== null && line.hits > 0 ? 'bg-success-soft/40' : '')}
          >
            <span className="w-12 shrink-0 border-r border-border px-2 text-right text-fg-subtle select-none">{line.line}</span>
            <span className={cn('w-16 shrink-0 border-r border-border px-2 text-right tabular-nums select-none', line.hits === 0 ? 'text-danger' : 'text-fg-muted')}>
              {line.hits === null ? '' : t('detail.hits', { count: line.hits })}
            </span>
            <span className="w-12 shrink-0 border-r border-border px-1.5 text-right text-[10px] text-fg-subtle select-none">
              {line.branches ? (
                <Tooltip content={t('detail.branchesOnLine', { taken: line.branches[0], total: line.branches[1] })}>
                  <span className={line.branches[0] < line.branches[1] ? 'text-warning' : ''}>
                    {line.branches[0]}/{line.branches[1]}
                  </span>
                </Tooltip>
              ) : null}
            </span>
            <pre className="px-2 whitespace-pre text-fg" data-selectable>
              {hasText ? line.text : t('detail.noSourceLine')}
            </pre>
          </div>
        ))}
      </div>
      {detail.functions.length > 0 && (
        <section className="space-y-1">
          <p className="text-xs font-medium text-fg-muted">{t('detail.functions', { count: detail.functions.length })}</p>
          <ul className="divide-y divide-border rounded-control border border-border text-xs">
            {detail.functions.map((fn) => (
              <li key={fn.name} className="flex items-center gap-2 px-2.5 py-1">
                <span className={cn('w-14 shrink-0 text-right tabular-nums', fn.hits === 0 ? 'text-danger' : 'text-fg-muted')}>{t('detail.hits', { count: fn.hits })}</span>
                <code className="min-w-0 flex-1 truncate text-fg" title={fn.name}>
                  {fn.name}
                </code>
                {fn.line !== null && (
                  <button type="button" className="text-fg-subtle underline tabular-nums" onClick={() => jump(fn.line ?? 0)}>
                    {fn.endLine ? `${fn.line}–${fn.endLine}` : fn.line}
                  </button>
                )}
              </li>
            ))}
          </ul>
        </section>
      )}
      {detail.branches.some((b) => !b.taken) && (
        <section className="space-y-1">
          <p className="text-xs font-medium text-fg-muted">{t('detail.untakenBranches')}</p>
          <ul className="flex flex-wrap gap-1 text-xs">
            {detail.branches
              .filter((b) => !b.taken)
              .slice(0, 200)
              .map((b) => (
                <li key={`${b.line}-${b.block}-${b.branch}`}>
                  <button type="button" onClick={() => jump(b.line)} className="rounded border border-border px-1.5 py-0.5 font-mono tabular-nums">
                    {t(b.taken === null ? 'detail.branchNotExecuted' : 'detail.branchNotTaken', { line: b.line, block: b.block, branch: b.branch })}
                  </button>
                </li>
              ))}
          </ul>
        </section>
      )}
    </div>
  )
}

function LcovViewer() {
  const { t, call, errorMessage } = usePlugin()
  const loading = useTask<Overview>()
  // 打开新报告时 result 会先清空，界面回到读取进度
  const overview = loading.result
  const [lcovPath, setLcovPath] = useState<string | null>(null)
  const [kind, setKind] = useState<Kind>('files')
  const [sort, setSort] = useState<Sort>('lines')
  const [ascending, setAscending] = useState(true)
  const [query, setQuery] = useState('')
  const [page, setPage] = useState(0)
  const [selected, setSelected] = useState<string | null>(null)
  const [detail, setDetail] = useState<Detail | null>(null)
  const [detailBusy, setDetailBusy] = useState(false)
  const [sourceNote, setSourceNote] = useState<string | null>(null)
  const [root, setRoot] = usePluginState('sourceRoot', '')
  const [mappings, setMappings] = usePluginState<Mapping[]>('pathMappings', [])
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [diagnosticsOpen, setDiagnosticsOpen] = useState(false)
  const [dropping, setDropping] = useState(false)

  const load = (path: string) => {
    setLcovPath(path)
    setSelected(null)
    setDetail(null)
    setPage(0)
    void loading.start('load', { path })
  }

  useEffect(() => {
    if (loading.status === 'failed' && loading.error) toast.error(errorMessage(loading.error))
  }, [loading.status]) // eslint-disable-line react-hooks/exhaustive-deps

  const pick = async () => {
    try {
      const path = await host.dialog.openFile([{ name: t('open.filter'), extensions: ['info', 'lcov', 'txt'] }])
      if (path) load(path)
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const loadRef = useRef(load)
  useEffect(() => {
    loadRef.current = load
  })
  useEffect(() => {
    const off = host.onFileDrop({ drop: (paths) => paths[0] && loadRef.current(paths[0]), over: setDropping })
    return () => {
      off.then((unlisten) => unlisten())
    }
  }, [])

  const rows = useMemo(() => {
    if (!overview) return [] as Entry[]
    const source = kind === 'files' ? overview.files : overview.dirs
    const needle = query.trim().toLowerCase()
    const filtered = needle ? source.filter((entry) => entry.path.toLowerCase().includes(needle)) : source
    const key = (entry: Entry) => (sort === 'path' ? 0 : (ratio(entry[sort]) ?? 101))
    return [...filtered].sort((a, b) => {
      const diff = sort === 'path' ? a.path.localeCompare(b.path) : key(a) - key(b) || a.path.localeCompare(b.path)
      return ascending ? diff : -diff
    })
  }, [overview, kind, query, sort, ascending])
  const pages = Math.max(1, Math.ceil(rows.length / PAGE))
  const visible = rows.slice(page * PAGE, page * PAGE + PAGE)
  const short = (path: string) => (overview && path.startsWith(overview.commonPrefix) ? path.slice(overview.commonPrefix.length) || path : path)

  const open = async (file: string) => {
    if (!overview) return
    setSelected(file)
    setDetailBusy(true)
    setSourceNote(null)
    let source: string | undefined
    if (root.trim()) {
      try {
        const resolved = await call<{ path: string }>('resolve', { path: file, root, mappings: mappings.filter((m) => m.from.trim()) })
        source = await host.fs.readText(resolved.path)
      } catch (reason) {
        setSourceNote(t('source.unavailable', { reason: errorMessage(reason) }))
      }
    } else {
      setSourceNote(t('source.noRoot'))
    }
    try {
      setDetail(await call<Detail>('detail', { token: overview.token, file, source }))
    } catch (reason) {
      const message = errorMessage(reason)
      // 插件实例重建后缓存失效：重新加载报告
      if ((reason as { code?: string }).code === 'lcov.expired' && lcovPath) {
        toast.info(t('open.reloading'))
        load(lcovPath)
      } else {
        toast.error(message)
      }
    } finally {
      setDetailBusy(false)
    }
  }

  const exportReport = async (format: 'csv' | 'markdown', target: 'copy' | 'save') => {
    if (!overview) return
    try {
      const text = await call<string>('export', { token: overview.token, format })
      if (target === 'copy') {
        await host.clipboard.writeText(text)
        toast.success(t('common:copied'))
        return
      }
      const ext = format === 'csv' ? 'csv' : 'md'
      const path = await host.dialog.saveFile(`coverage.${ext}`, [{ name: format.toUpperCase(), extensions: [ext] }])
      if (!path) return
      await host.fs.writeText(path, text)
      toast.success(t('common:saved'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  if (!overview) {
    return (
      <Panel className="h-full" bodyClassName="flex h-full items-center justify-center p-3">
        {loading.running ? (
          <div className="w-72 space-y-2 text-center">
            <Progress value={loading.progress ? (loading.progress.done / Math.max(loading.progress.total, 1)) * 100 : null} aria-label={t('open.loading')} />
            <p className="text-xs text-fg-muted">{t('open.loading')}</p>
            <Button size="sm" onClick={() => void loading.cancel()}>
              {t('open.cancel')}
            </Button>
          </div>
        ) : (
          <button
            type="button"
            onClick={pick}
            className={cn(
              'flex h-full min-h-64 w-full flex-col items-center justify-center gap-2 rounded-card border-2 border-dashed text-center outline-none transition-colors',
              'focus-visible:ring-2 focus-visible:ring-ring',
              dropping ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg-muted hover:border-border-strong hover:bg-hover',
            )}
          >
            <Upload className="size-6" />
            <span className="text-[13px] font-medium">{t('open.drop')}</span>
            <span className="max-w-md text-xs text-fg-subtle">{t('open.hint')}</span>
          </button>
        )}
      </Panel>
    )
  }

  const diagnosticCount = overview.diagnostics.length + overview.diagnosticsDropped
  const ignored = Object.entries(overview.ignored)

  return (
    <div className={cn('flex h-full min-h-0 flex-col gap-3', dropping && 'rounded-card ring-2 ring-primary')}>
      <div className="flex shrink-0 flex-wrap items-center gap-2">
        <div className="min-w-0 flex-1">
          <Metrics3 metrics={overview.totals} />
        </div>
        <div className="flex flex-col items-end gap-1.5">
          <div className="flex items-center gap-1.5">
            <Button size="sm" onClick={() => void pick()} loading={loading.running}>
              <FolderOpen />
              {t('open.other')}
            </Button>
            <Tooltip content={t('source.title')}>
              <Button size="icon-sm" variant="ghost" aria-label={t('source.title')} onClick={() => setSettingsOpen(true)}>
                <Settings2 />
              </Button>
            </Tooltip>
            <DropdownMenu
              trigger={
                <Button size="sm">
                  <Download />
                  {t('export.title')}
                  <ChevronDown />
                </Button>
              }
              items={[
                { key: 'copy-md', label: t('export.copyMarkdown'), onSelect: () => void exportReport('markdown', 'copy') },
                { key: 'save-md', label: t('export.saveMarkdown'), onSelect: () => void exportReport('markdown', 'save') },
                { key: 'copy-csv', label: t('export.copyCsv'), onSelect: () => void exportReport('csv', 'copy') },
                { key: 'save-csv', label: t('export.saveCsv'), onSelect: () => void exportReport('csv', 'save') },
              ]}
            />
          </div>
          <div className="flex flex-wrap justify-end gap-1 text-[11px] text-fg-subtle">
            <span>{t('summary.files', { count: overview.files.length, records: overview.records })}</span>
            {overview.tests.length > 0 && <span>· {t('summary.tests', { list: overview.tests.slice(0, 3).join(', '), count: overview.tests.length })}</span>}
            {diagnosticCount + ignored.length > 0 && (
              <button type="button" className="text-warning underline" onClick={() => setDiagnosticsOpen(true)}>
                · {t('summary.diagnostics', { count: diagnosticCount + ignored.length })}
              </button>
            )}
          </div>
        </div>
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-[minmax(320px,0.9fr)_minmax(0,1.4fr)] gap-3">
        <Panel
          icon={<FolderTree />}
          title={t('table.title')}
          actions={
            <SegmentedControl<Kind>
              size="sm"
              value={kind}
              onValueChange={(value) => {
                setKind(value)
                setPage(0)
              }}
              aria-label={t('table.kind')}
              options={[
                { value: 'files', label: t('table.files') },
                { value: 'dirs', label: t('table.dirs') },
              ]}
            />
          }
          footer={
            <div className="flex items-center justify-between gap-2">
              <span className="text-xs text-fg-subtle tabular-nums">{t('table.count', { count: rows.length })}</span>
              {pages > 1 && (
                <span className="flex items-center gap-1 text-xs tabular-nums">
                  <Button size="icon-sm" variant="ghost" aria-label={t('table.previous')} disabled={page === 0} onClick={() => setPage(page - 1)}>
                    <ChevronLeft />
                  </Button>
                  {page + 1} / {pages}
                  <Button size="icon-sm" variant="ghost" aria-label={t('table.next')} disabled={page + 1 >= pages} onClick={() => setPage(page + 1)}>
                    <ChevronRight />
                  </Button>
                </span>
              )}
            </div>
          }
          bodyClassName="flex min-h-0 flex-col gap-2 p-2"
        >
          <div className="flex items-center gap-1.5">
            <Input
              size="sm"
              className="flex-1"
              value={query}
              onChange={(event) => {
                setQuery(event.target.value)
                setPage(0)
              }}
              placeholder={t('table.search')}
              aria-label={t('table.search')}
            />
            <Select<Sort>
              size="sm"
              value={sort}
              onValueChange={setSort}
              aria-label={t('table.sort')}
              options={(['lines', 'functions', 'branches', 'path'] as Sort[]).map((value) => ({ value, label: t(`table.sorts.${value}`) }))}
            />
            <Button size="sm" variant="ghost" onClick={() => setAscending(!ascending)}>
              {t(ascending ? 'table.ascending' : 'table.descending')}
            </Button>
          </div>
          <ul className="min-h-0 flex-1 space-y-0.5 overflow-auto">
            {visible.map((entry) => (
              <li key={entry.path}>
                <button
                  type="button"
                  disabled={kind === 'dirs'}
                  onClick={() => void open(entry.path)}
                  onDoubleClick={() => {
                    if (kind === 'dirs') {
                      setKind('files')
                      setQuery(entry.path)
                      setPage(0)
                    }
                  }}
                  className={cn(
                    'w-full space-y-1 rounded-control px-2 py-1.5 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring',
                    selected === entry.path ? 'bg-primary-soft' : 'hover:bg-hover',
                  )}
                >
                  <div className="flex items-center gap-2 text-xs">
                    <span className="min-w-0 flex-1 truncate font-mono text-fg" title={entry.path}>
                      {short(entry.path)}
                      {kind === 'dirs' && <span className="ml-1 text-fg-subtle">({entry.files})</span>}
                    </span>
                    <span className={cn('shrink-0 tabular-nums', tone(entry.lines))}>{pctText(entry.lines)}</span>
                    <span className={cn('w-12 shrink-0 text-right tabular-nums', tone(entry.functions))}>{pctText(entry.functions)}</span>
                    <span className={cn('w-12 shrink-0 text-right tabular-nums', tone(entry.branches))}>{pctText(entry.branches)}</span>
                  </div>
                  <Bar counts={entry.lines} />
                </button>
              </li>
            ))}
          </ul>
          {kind === 'dirs' && <p className="text-[11px] text-fg-subtle">{t('table.dirsHint')}</p>}
        </Panel>

        <Panel icon={<FileCode2 />} title={selected ? short(selected) : t('detail.title')} extra={detailBusy && <Spinner className="size-3.5 text-fg-subtle" />} bodyClassName="overflow-auto p-3">
          {detail && selected === detail.path ? (
            <FileDetail detail={detail} sourceNote={sourceNote} />
          ) : (
            <Empty icon={<FileCode2 />} title={t('detail.empty')} description={t('detail.emptyHint')} />
          )}
        </Panel>
      </div>

      <Settings open={settingsOpen} onClose={() => setSettingsOpen(false)} root={root} setRoot={setRoot} mappings={mappings} setMappings={setMappings} />

      <Modal open={diagnosticsOpen} onOpenChange={setDiagnosticsOpen} size="lg" title={t('diagnostics.title')} closeLabel={t('source.close')}>
        <div className="max-h-[60vh] space-y-2 overflow-auto text-xs">
          {ignored.length > 0 && (
            <p className="flex flex-wrap items-center gap-1 text-fg-muted">
              <ShieldAlert className="size-3.5" />
              {t('diagnostics.ignored')}
              {ignored.map(([kind, count]) => (
                <Badge key={kind}>{t('diagnostics.ignoredCount', { kind, count })}</Badge>
              ))}
            </p>
          )}
          <ul className="space-y-1">
            {overview.diagnostics.map((d, index) => (
              <li key={index} className="flex gap-2">
                <span className="w-16 shrink-0 text-right text-fg-subtle tabular-nums">{d.line !== undefined && t('diagnostics.line', { line: d.line })}</span>
                <span className="text-fg">
                  {t(`diagnostics.${d.kind}`)}
                  {d.file && <code className="ml-1 text-fg-muted">{d.file}</code>}
                  {d.detail && <code className="ml-1 text-fg-muted">{d.detail}</code>}
                </span>
              </li>
            ))}
          </ul>
          {overview.diagnosticsDropped > 0 && <p className="text-fg-subtle">{t('diagnostics.dropped', { count: overview.diagnosticsDropped })}</p>}
        </div>
      </Modal>
    </div>
  )
}

export default LcovViewer
