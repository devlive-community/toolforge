import { useEffect, useRef, useState } from 'react'
import { Badge, Button, Input, Panel, Progress, Switch, Tooltip, cn, toast } from '@toolforge/ui'
import { CopyButton, formatBytes, host, usePlugin, usePluginState, useTask } from '@toolforge/plugin-ui-sdk'
import { CaseSensitive, FileText, FolderOpen, History, Regex, RotateCw, Search, Upload, X } from 'lucide-react'
import { LEVEL_BAR, LogList, type Growth } from './LogList'
import { LEVELS, type Detail, type FilterResult, type Level, type Line, type OpenInfo, type Refreshed, type Stats } from './types'

const FILTER_DELAY = 300
const FOLLOW_INTERVAL = 1000
const MAX_RECENT = 8
const baseName = (path: string) => path.split(/[\\/]/).pop() ?? path

function IconToggle({ pressed, onChange, label, children }: { pressed: boolean; onChange: (value: boolean) => void; label: string; children: React.ReactNode }) {
  return (
    <Tooltip content={label}>
      <Button size="icon-sm" variant={pressed ? 'primary' : 'ghost'} aria-pressed={pressed} aria-label={label} onClick={() => onChange(!pressed)}>
        {children}
      </Button>
    </Tooltip>
  )
}

function LogViewer() {
  const { t, call, errorMessage } = usePlugin()
  const opening = useTask<OpenInfo>()
  const filtering = useTask<FilterResult>()
  const [recent, setRecent] = usePluginState<string[]>('recent', [])
  const [doc, setDoc] = useState<OpenInfo | null>(null)
  const [stats, setStats] = useState<Stats | null>(null)
  const [query, setQuery] = useState('')
  const [regex, setRegex] = useState(false)
  const [caseSensitive, setCaseSensitive] = useState(false)
  const [levels, setLevels] = useState<Level[]>([])
  const [view, setView] = useState<{ id: number; total: number } | null>(null)
  const [follow, setFollow] = useState(false)
  const [epoch, setEpoch] = useState(0)
  const [growth, setGrowth] = useState<Growth>({ from: 0, seq: 0 })
  const [detail, setDetail] = useState<Detail | null>(null)
  const [hovering, setHovering] = useState(false)
  const lastFilter = useRef('')
  /** 当前视图的行数，文件增长时据此判断需要重新读取的块 */
  const shown = useRef(0)

  const open = (path: string) => {
    setFollow(false)
    setDetail(null)
    opening.start('open', { path })
    setRecent([path, ...recent.filter((p) => p !== path)].slice(0, MAX_RECENT))
  }

  // 打开完成后切换到新文件
  const opened = opening.status === 'succeeded' ? opening.result : null
  const [seenOpen, setSeenOpen] = useState<OpenInfo | null>(null)
  if (opened && opened !== seenOpen) {
    setSeenOpen(opened)
    setDoc(opened)
    setStats(opened)
    // 新文件的 id 不同，筛选条件会自动重新应用
    setView(null)
  }

  useEffect(() => {
    for (const task of [opening, filtering]) {
      if (task.status === 'failed' && task.error) toast.error(errorMessage(task.error))
    }
  }, [opening.status, opening.error, filtering.status, filtering.error, errorMessage]) // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    const off = host.onFileDrop({ drop: (paths) => paths[0] && open(paths[0]), over: setHovering })
    return () => {
      off.then((unlisten) => unlisten())
    }
    // open 只使用稳定的状态设置函数
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [recent])

  // 筛选条件变化后防抖运行筛选任务；条件为空时直接显示全部行
  const filterKey = doc ? JSON.stringify({ id: doc.id, query, regex, caseSensitive, levels }) : ''
  useEffect(() => {
    if (!doc || filterKey === lastFilter.current) return
    const timer = setTimeout(() => {
      lastFilter.current = filterKey
      if (filtering.running) void filtering.cancel()
      if (!query && levels.length === 0) {
        setView(null)
        return
      }
      filtering.start('filter', { id: doc.id, query, regex, caseSensitive, levels })
    }, FILTER_DELAY)
    return () => clearTimeout(timer)
  }, [filterKey]) // eslint-disable-line react-hooks/exhaustive-deps

  const filtered = filtering.status === 'succeeded' ? filtering.result : null
  const [seenFilter, setSeenFilter] = useState<FilterResult | null>(null)
  if (filtered && filtered !== seenFilter) {
    setSeenFilter(filtered)
    setView(filtered.view === 0 ? null : { id: filtered.view, total: filtered.total })
  }

  const apply = (result: Refreshed) => {
    if (!result.changed) return
    setStats(result.stats)
    if (result.reset) setEpoch((e) => e + 1)
    else setGrowth((g) => ({ from: Math.max(0, shown.current - 1), seq: g.seq + 1 }))
    setView((prev) => (prev && result.viewTotal !== null ? { ...prev, total: result.viewTotal } : prev))
  }
  const applyRef = useRef(apply)
  useEffect(() => {
    applyRef.current = apply
  })

  // 跟随模式：定时检查文件增长
  const docId = doc?.id
  useEffect(() => {
    if (!follow || docId === undefined) return
    let alive = true
    const timer = setInterval(() => {
      call<Refreshed>('refresh', { id: docId })
        .then((result) => {
          if (alive) applyRef.current(result)
        })
        .catch((error) => {
          setFollow(false)
          toast.error(errorMessage(error))
        })
    }, FOLLOW_INTERVAL)
    return () => {
      alive = false
      clearInterval(timer)
    }
  }, [follow, docId, call, errorMessage])

  const reload = () => {
    if (docId === undefined) return
    call<Refreshed>('refresh', { id: docId })
      .then((result) => {
        apply(result)
        toast.success(result.changed ? t('toolbar.reloaded') : t('toolbar.unchanged'))
      })
      .catch((error) => toast.error(errorMessage(error)))
  }

  const select = (line: Line) => {
    if (!doc) return
    call<Detail>('line', { id: doc.id, line: line.n })
      .then(setDetail)
      .catch((error) => toast.error(errorMessage(error)))
  }

  const pick = async () => {
    try {
      const path = await host.dialog.openFile([{ name: t('open.filter'), extensions: ['log', 'txt', 'out', 'err', 'json', 'jsonl', 'ndjson', 'csv', '*'] }])
      if (path) open(path)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const toggleLevel = (level: Level) => setLevels((prev) => (prev.includes(level) ? prev.filter((l) => l !== level) : [...prev, level]))
  const progress = (task: { progress: { done: number; total: number } | null }) =>
    task.progress && task.progress.total > 0 ? (task.progress.done / task.progress.total) * 100 : null

  const total = view ? view.total : (stats?.lines ?? 0)
  useEffect(() => {
    shown.current = total
  }, [total])

  if (!doc || !stats) {
    return (
      <div className="flex h-full min-h-0 flex-col gap-3">
        <button
          type="button"
          onClick={pick}
          disabled={opening.running}
          className={cn(
            'flex min-h-56 flex-1 flex-col items-center justify-center gap-2 rounded-card border-2 border-dashed text-center outline-none transition-colors',
            'focus-visible:ring-2 focus-visible:ring-ring',
            hovering ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg-muted hover:border-border-strong hover:bg-hover',
          )}
        >
          <Upload className="size-6" />
          <span className="text-[13px] font-medium">{opening.running ? t('open.indexing') : t('open.drop')}</span>
          {opening.running ? (
            <span className="w-64">
              <Progress value={progress(opening)} />
            </span>
          ) : (
            <span className="text-xs text-fg-subtle">{t('open.dropHint')}</span>
          )}
        </button>
        {recent.length > 0 && (
          <Panel icon={<History />} title={t('open.recent')} className="shrink-0" bodyClassName="p-2">
            <ul className="space-y-0.5">
              {recent.map((path) => (
                <li key={path} className="group flex items-center gap-2 rounded-control px-2.5 py-1.5 hover:bg-hover">
                  <button type="button" className="min-w-0 flex-1 text-left outline-none" onClick={() => open(path)} disabled={opening.running}>
                    <p className="truncate text-[13px] text-fg">{baseName(path)}</p>
                    <p className="truncate text-[11px] text-fg-subtle">{path}</p>
                  </button>
                  <Tooltip content={t('open.forget')}>
                    <Button
                      size="icon-sm"
                      variant="ghost"
                      aria-label={t('open.forget')}
                      className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                      onClick={() => setRecent(recent.filter((p) => p !== path))}
                    >
                      <X />
                    </Button>
                  </Tooltip>
                </li>
              ))}
            </ul>
          </Panel>
        )}
      </div>
    )
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <section className="flex flex-col gap-2.5 rounded-card border border-border bg-surface p-3 shadow-card">
        <div className="flex flex-wrap items-center gap-2">
          <Input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t('toolbar.search')}
            aria-label={t('toolbar.search')}
            leading={<Search />}
            className="font-mono"
            spellCheck={false}
            wrapperClassName="min-w-60 flex-1"
            trailing={
              <span className="flex items-center gap-0.5">
                <IconToggle pressed={caseSensitive} onChange={setCaseSensitive} label={t('toolbar.caseSensitive')}>
                  <CaseSensitive />
                </IconToggle>
                <IconToggle pressed={regex} onChange={setRegex} label={t('toolbar.regex')}>
                  <Regex />
                </IconToggle>
              </span>
            }
          />
          <label className="flex items-center gap-2 text-[13px] text-fg">
            <Switch size="sm" checked={follow} onCheckedChange={setFollow} aria-label={t('toolbar.follow')} />
            {t('toolbar.follow')}
          </label>
          <Tooltip content={t('toolbar.reload')}>
            <Button size="icon-md" variant="ghost" aria-label={t('toolbar.reload')} onClick={reload}>
              <RotateCw />
            </Button>
          </Tooltip>
          <Button onClick={pick} disabled={opening.running}>
            <FolderOpen />
            {t('toolbar.open')}
          </Button>
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          {LEVELS.filter((level) => stats.counts[level] > 0 || levels.includes(level)).map((level) => {
            const active = levels.includes(level)
            return (
              <button
                key={level}
                type="button"
                aria-pressed={active}
                onClick={() => toggleLevel(level)}
                className={cn(
                  'flex h-6 items-center gap-1.5 rounded-full border px-2.5 text-xs outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring',
                  active ? 'border-primary bg-primary-soft text-fg' : 'border-border text-fg-muted hover:bg-hover',
                )}
              >
                <span className={cn('size-2 rounded-full', level === 'none' ? 'bg-border-strong' : LEVEL_BAR[level])} />
                {t(`levels.${level}`)}
                <span className="text-fg-subtle tabular-nums">{stats.counts[level].toLocaleString()}</span>
              </button>
            )
          })}
          {levels.length > 0 && (
            <Button size="sm" variant="ghost" onClick={() => setLevels([])}>
              {t('toolbar.allLevels')}
            </Button>
          )}
          {(opening.running || filtering.running) && (
            <span className="ml-auto w-40">
              <Progress value={progress(opening.running ? opening : filtering)} />
            </span>
          )}
        </div>
      </section>

      <div className={cn('grid min-h-0 flex-1 gap-3', detail ? 'grid-cols-[minmax(0,1fr)_360px]' : 'grid-cols-1')}>
        <Panel
          icon={<FileText />}
          title={doc.name}
          extra={<Badge>{doc.encoding}</Badge>}
          footer={
            <>
              <span className="shrink-0 whitespace-nowrap tabular-nums">
                {view ? t('status.filtered', { shown: total.toLocaleString(), total: stats.lines.toLocaleString() }) : t('status.lines', { total: stats.lines.toLocaleString() })}
              </span>
              <span className="ml-auto truncate tabular-nums" title={doc.path}>
                {`${formatBytes(stats.bytes)} · ${doc.path}`}
              </span>
            </>
          }
          bodyClassName="p-0"
        >
          <LogList
            docId={doc.id}
            view={view?.id ?? 0}
            epoch={epoch}
            total={total}
            lineCount={stats.lines}
            growth={growth}
            follow={follow}
            selected={detail?.n ?? null}
            onSelect={select}
            onScrolledAway={() => setFollow(false)}
          />
        </Panel>
        {detail && (
          <Panel
            title={t('detail.title', { n: detail.n.toLocaleString() })}
            extra={<Badge>{t(`levels.${detail.level}`)}</Badge>}
            actions={
              <>
                <CopyButton text={detail.text} />
                <Button size="icon-sm" variant="ghost" aria-label={t('detail.close')} onClick={() => setDetail(null)}>
                  <X />
                </Button>
              </>
            }
            bodyClassName="space-y-3 overflow-auto p-3"
          >
            <pre className="font-mono text-[12px] leading-relaxed break-all whitespace-pre-wrap text-fg" data-selectable>
              {detail.text}
            </pre>
            {detail.truncated && <p className="text-xs text-warning">{t('detail.truncated')}</p>}
            {detail.json && (
              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <p className="text-xs font-medium text-fg-muted">{t('detail.json')}</p>
                  <CopyButton text={detail.json} />
                </div>
                <pre className="overflow-auto rounded-control bg-surface-2 p-2.5 font-mono text-[12px] leading-relaxed text-fg" data-selectable>
                  {detail.json}
                </pre>
              </div>
            )}
          </Panel>
        )}
      </div>
    </div>
  )
}

export default LogViewer
