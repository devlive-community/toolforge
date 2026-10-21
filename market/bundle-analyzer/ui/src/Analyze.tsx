import { useEffect, useId, useRef, useState } from 'react'
import { host, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { Button, Input, cn, toast } from '@toolforge/ui'
import { ArrowLeft, Boxes, ChevronRight, Copy, Download } from 'lucide-react'
import { Busy, Bytes, Kind, Pager, PAGE, key, useRequest } from './Widgets'
import type { Import, Module, Output, Page, Query, Trace, Tree } from './types'

export function OutputPage({
  q,
  page,
  setPage,
  selected,
  select,
}: {
  q: Query
  page: number
  setPage: (n: number) => void
  selected: string | null
  select: (path: string) => void
}) {
  const { t } = usePlugin()
  const task = useRequest<Page<Output>>('query', { ...q, offset: (page - 1) * PAGE, limit: PAGE })
  return (
    <>
      <Busy task={task} />
      <div className="min-h-0 flex-1 space-y-2 overflow-auto">
        {task.result?.items.map((out) => (
          <button
            key={out.path}
            type="button"
            onClick={() => select(out.path)}
            className={cn(
              'w-full space-y-1 rounded-control border p-2.5 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring',
              selected === out.path ? 'border-primary bg-primary-soft' : 'border-border hover:bg-hover',
            )}
          >
            <p className="break-words font-mono text-xs text-fg">{out.path}</p>
            <p className="text-xs text-fg">
              <Bytes value={out.bytes} />
            </p>
            <p className="truncate text-[10px] text-fg-muted">{out.entryPoint ?? t('output.shared')}</p>
          </button>
        ))}
        {task.result?.total === 0 && <p className="text-xs text-fg-muted">{t('emptyRows')}</p>}
      </div>
      <Pager page={page} total={task.result?.total ?? 0} setPage={setPage} />
    </>
  )
}
function TreeMap({ q, navigate, select }: { q: Query; navigate: (directory: string[]) => void; select: (path: string) => void }) {
  const { t } = usePlugin()
  const task = useRequest<Tree>('query', { ...q, kind: 'tree' })
  const prefix = useId()
  return (
    <div className="flex h-full min-h-0 flex-col gap-2">
      <Busy task={task} />
      {task.result && (
        <>
          <p className="shrink-0 text-[10px] text-fg-muted">
            {t('tree.basis')} <Bytes value={task.result.basis} />
            {task.result.overAttributed > 0 && <span className="ml-2 text-warning">{t('tree.over')}</span>}
          </p>
          <svg
            role="group"
            aria-label={t('tree.title')}
            viewBox="0 0 1000 600"
            className="min-h-0 w-full flex-1 rounded-control border border-border bg-surface-2"
            preserveAspectRatio="none"
          >
            {task.result.items.map((tile, i) => (
              <g
                key={i}
                role={tile.directory || tile.path ? 'button' : undefined}
                tabIndex={tile.directory || tile.path ? 0 : undefined}
                aria-label={`${tile.name || t(`tree.${tile.kind}`)} ${t('bytes', { value: tile.bytes.toLocaleString() })}`}
                className={tile.directory || tile.path ? 'cursor-pointer outline-none focus:stroke-primary' : ''}
                onClick={() => {
                  if (tile.directory) navigate(tile.directory)
                  else if (tile.path) select(tile.path)
                }}
                onKeyDown={(event) => {
                  if (event.key === 'Enter' || event.key === ' ') {
                    event.preventDefault()
                    if (tile.directory) navigate(tile.directory)
                    else if (tile.path) select(tile.path)
                  }
                }}
              >
                <title>
                  {tile.name || t(`tree.${tile.kind}`)} · {t('bytes', { value: tile.bytes.toLocaleString() })}
                </title>
                <rect
                  x={tile.x}
                  y={tile.y}
                  width={tile.width}
                  height={tile.height}
                  strokeWidth={2}
                  className={cn(
                    'stroke-surface',
                    tile.kind === 'unattributed'
                      ? 'fill-warning/25'
                      : tile.kind === 'other'
                        ? 'fill-fg-muted/20'
                        : tile.kind === 'directory'
                          ? 'fill-primary/35'
                          : 'fill-primary/20',
                  )}
                />
                <clipPath id={`${prefix}-${i}`}>
                  <rect x={tile.x + 5} y={tile.y + 4} width={Math.max(0, tile.width - 10)} height={Math.max(0, tile.height - 8)} />
                </clipPath>
                {tile.width > 70 && tile.height > 45 && (
                  <text x={tile.x + 8} y={tile.y + 23} clipPath={`url(#${prefix}-${i})`} className="fill-fg" fontSize={25}>
                    {tile.name || t(`tree.${tile.kind}`)}
                    <tspan x={tile.x + 8} dy={23}>
                      {t('bytes', { value: tile.bytes.toLocaleString() })}
                    </tspan>
                  </text>
                )}
              </g>
            ))}
          </svg>
          {(task.result.limited || task.result.depthLimited) && <p className="text-[10px] text-warning">{t('tree.limited')}</p>}
        </>
      )}
    </div>
  )
}
function Modules({
  q,
  page,
  setPage,
  selected,
  select,
}: {
  q: Query
  page: number
  setPage: (n: number) => void
  selected: string | null
  select: (path: string) => void
}) {
  const { t } = usePlugin()
  const task = useRequest<Page<Module>>('query', { ...q, kind: 'modules', offset: (page - 1) * PAGE, limit: PAGE })
  return (
    <div className="flex h-full min-h-0 flex-col gap-2">
      <Busy task={task} />
      <p className="shrink-0 text-[10px] text-fg-subtle">{t('modules.total', { count: task.result?.total ?? 0 })}</p>
      <div className="min-h-0 flex-1 space-y-1 overflow-auto">
        {task.result?.items.map((m) => (
          <button
            type="button"
            key={m.path}
            onClick={() => select(m.path)}
            className={cn(
              'w-full rounded-control border p-2 text-left hover:bg-hover',
              selected === m.path ? 'border-primary bg-primary-soft' : 'border-border',
            )}
          >
            <div className="flex items-center gap-2">
              <span className="min-w-0 flex-1 break-words font-mono text-xs text-fg">{m.path}</span>
              <span className="shrink-0 text-xs text-fg">
                <Bytes value={m.bytes} />
              </span>
            </div>
            <p className="mt-1 text-[10px] text-fg-subtle">
              {t('modules.source')} <Bytes value={m.sourceBytes} />
              {!m.known && <span className="ml-2 text-warning">{t('modules.unknown')}</span>}
            </p>
          </button>
        ))}
        {task.result?.total === 0 && <p className="text-xs text-fg-muted">{t('emptyRows')}</p>}
      </div>
      <Pager page={page} total={task.result?.total ?? 0} setPage={setPage} />
    </div>
  )
}
function ImportPage({ q, page, setPage }: { q: Query; page: number; setPage: (n: number) => void }) {
  const { t } = usePlugin()
  const task = useRequest<Page<Import>>('query', { ...q, kind: 'imports', offset: (page - 1) * PAGE, limit: PAGE })
  return (
    <>
      <Busy task={task} />
      <ul className="space-y-2">
        {task.result?.items.map((item, i) => (
          <li key={i} className="space-y-1 rounded-control border border-border p-2">
            <p className="break-words font-mono text-xs text-fg">{item.path}</p>
            <div className="flex items-center gap-2">
              <Kind external={item.external} resolved={item.resolved} />
              <code className="text-[10px] text-fg-muted">{item.kind}</code>
            </div>
          </li>
        ))}
      </ul>
      {task.result?.total === 0 && <p className="text-xs text-fg-muted">{t('imports.empty')}</p>}
      <Pager page={page} total={task.result?.total ?? 0} setPage={setPage} />
    </>
  )
}
function TraceView({ q }: { q: Query }) {
  const { t, errorMessage } = usePlugin()
  const task = useRequest<Trace>('trace', { ...q, kind: 'trace' })
  const transfer = useTask<{ content?: string; saved?: boolean }>()
  const handled = useRef<string | null>(null)
  useEffect(() => {
    if (!transfer.result?.content || handled.current === transfer.taskId) return
    handled.current = transfer.taskId
    void host.clipboard
      .writeText(transfer.result.content)
      .then(() => toast.success(t('export.copied')))
      .catch((e) => toast.error(errorMessage(e)))
  }, [transfer.result, transfer.taskId, t, errorMessage])
  const saveTrace = async (copy: boolean) => {
    try {
      const path = copy ? null : await host.dialog.saveFile('bundle-import-path.json', [{ name: t('export.json'), extensions: ['json'] }])
      if (!copy && !path) return
      await transfer.start(copy ? 'copy' : 'export', { query: { ...q, kind: 'trace' }, format: copy ? 'markdown' : 'json', path })
    } catch (e) {
      toast.error(errorMessage(e))
    }
  }

  return (
    <div className="space-y-2">
      <Busy task={task} />
      <Busy task={transfer} />
      {task.result && (
        <>
          <div className="flex flex-wrap gap-2">
            <Button size="sm" disabled={transfer.running} onClick={() => void saveTrace(true)}>
              <Copy />
              {t('trace.copy')}
            </Button>
            <Button size="sm" disabled={transfer.running} onClick={() => void saveTrace(false)}>
              <Download />
              {t('trace.export')}
            </Button>
          </div>
          <p className="text-xs font-medium text-fg">{t(`trace.${task.result.state}`)}</p>
          {task.result.root && <code className="block break-words text-[11px] text-fg-muted">{task.result.root}</code>}
          <ol className="space-y-1">
            {task.result.edges.map((edge, i) => (
              <li key={i} className="flex gap-2 rounded-control border border-border p-2 text-[11px]">
                <ChevronRight className="size-3.5 shrink-0 text-fg-subtle" />
                <span className="min-w-0 break-words font-mono text-fg">
                  {edge.to}
                  <span className="ml-2 text-fg-subtle">{edge.kind}</span>
                </span>
              </li>
            ))}
          </ol>
          <p className="text-[10px] leading-relaxed text-fg-subtle">{t('trace.boundary')}</p>
        </>
      )}
    </div>
  )
}
export function Analyze({
  q,
  directory,
  navigate,
  search,
  setSearch,
}: {
  q: Query
  directory: string[]
  navigate: (d: string[]) => void
  search: string
  setSearch: (s: string) => void
}) {
  const { t } = usePlugin()
  const [page, setPage] = useState(1)
  const [selected, setSelected] = useState<string | null>(null)
  const [importPage, setImportPage] = useState(1)
  const [pane, setPane] = useState<'modules' | 'imports' | 'trace'>('modules')
  const meta = useRequest<Page<Output>>('query', { ...q, kind: 'output' })
  const scope = { ...q, directory, search }
  const select = (path: string) => {
    setSelected(path)
    setImportPage(1)
    setPane('trace')
  }
  const openDirectory = (d: string[]) => {
    setPage(1)
    navigate(d)
  }
  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <Busy task={meta} />
      {meta.result?.items[0] && (
        <div className="grid shrink-0 grid-cols-3 gap-2">
          {(['bytes', 'attributed', 'unattributed'] as const).map((field) => (
            <div key={field} className="rounded-control border border-border p-2">
              <p className="text-[10px] text-fg-muted">{t(`output.${field}`)}</p>
              <p className="text-sm text-fg">
                <Bytes value={meta.result!.items[0][field]} />
              </p>
            </div>
          ))}
          {meta.result.items[0].overAttributed > 0 && (
            <p className="col-span-3 text-xs text-warning">
              {t('output.over')} <Bytes value={meta.result.items[0].overAttributed} />
            </p>
          )}
        </div>
      )}
      <div className="flex shrink-0 flex-wrap gap-2">
        <Button size="sm" variant={pane === 'modules' ? 'secondary' : 'ghost'} onClick={() => setPane('modules')}>
          <Boxes />
          {t('modules.title')}
        </Button>
        <Button
          size="sm"
          variant={pane === 'imports' ? 'secondary' : 'ghost'}
          onClick={() => {
            setPane('imports')
            setSelected(null)
            setImportPage(1)
          }}
        >
          {t('imports.output')}
        </Button>
        {selected && (
          <Button size="sm" variant={pane === 'trace' ? 'secondary' : 'ghost'} onClick={() => setPane('trace')}>
            {t('trace.title')}
          </Button>
        )}
      </div>
      {pane === 'modules' ? (
        <>
          <div className="flex shrink-0 items-center gap-2">
            <Button size="sm" variant="ghost" disabled={directory.length === 0} onClick={() => openDirectory(directory.slice(0, -1))}>
              <ArrowLeft />
              {t('tree.up')}
            </Button>
            <span className="min-w-0 flex-1 truncate font-mono text-xs text-fg-muted">{directory.join('/') || t('tree.root')}</span>
          </div>
          <Input
            size="sm"
            value={search}
            onChange={(e) => {
              setSearch(e.target.value)
              setPage(1)
            }}
            maxLength={512}
            placeholder={t('modules.search')}
            aria-label={t('modules.search')}
          />
          <div className="grid min-h-0 flex-1 grid-rows-2 gap-3 @min-[1050px]:grid-cols-2 @min-[1050px]:grid-rows-1">
            <TreeMap key={key(scope)} q={scope} navigate={openDirectory} select={select} />
            <Modules key={`${key(scope)}:${page}`} q={scope} page={page} setPage={setPage} selected={selected} select={select} />
          </div>
        </>
      ) : (
        <div className="min-h-0 flex-1 space-y-3 overflow-auto">
          {selected && (
            <div className="space-y-2 rounded-control border border-border p-2">
              <p className="break-words font-mono text-xs text-fg">{selected}</p>
              <Button size="sm" variant="ghost" onClick={() => setSelected(null)}>
                {t('trace.clear')}
              </Button>
            </div>
          )}
          {pane === 'trace' && selected && <TraceView key={key({ ...q, module: selected })} q={{ ...q, module: selected }} />}
          <h4 className="text-xs font-medium text-fg-muted">{t(selected ? 'imports.module' : 'imports.output')}</h4>
          <ImportPage key={`${key({ ...q, module: selected })}:${importPage}`} q={{ ...q, module: selected }} page={importPage} setPage={setImportPage} />
        </div>
      )}
    </div>
  )
}
