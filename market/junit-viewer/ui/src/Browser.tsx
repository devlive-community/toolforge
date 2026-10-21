import { useEffect, useRef, useState } from 'react'
import { usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { Badge, CodeEditor, Empty, Pagination, Panel, Spinner, cn } from '@toolforge/ui'
import { ChevronRight, FileCode2, FolderTree, TestTube2 } from 'lucide-react'
import type { Block, Case, Chunk, Detail, Diagnostic, Filter, Page, Source, Status, Suite, Summary } from './types'

export const PAGE = 25
export function useRequest<T>(fn: string, args: object) {
  const task = useTask<T>()
  const initial = useRef(args)
  const { start, cancel } = task
  useEffect(() => {
    void start(fn, initial.current)
    return () => {
      void cancel()
    }
  }, [fn, start, cancel])
  return task
}
export function Pager({ page, total, setPage, size = PAGE }: { page: number; total: number; setPage: (page: number) => void; size?: number }) {
  const { t } = usePlugin()
  return (
    <Pagination
      page={page}
      pageSize={size}
      total={total}
      onPageChange={setPage}
      prevLabel={t('page.previous')}
      nextLabel={t('page.next')}
      pageLabel={(value) => t('page.label', { value })}
      aria-label={t('page.title')}
    />
  )
}
export function StatusBadge({ status }: { status: Status }) {
  const { t } = usePlugin()
  return (
    <Badge variant={status === 'passed' ? 'success' : status === 'failure' || status === 'error' ? 'danger' : status === 'unknown' ? 'warning' : 'neutral'}>
      {t(`status.${status}`)}
    </Badge>
  )
}
export function Busy({ error, running }: { error: unknown; running: boolean }) {
  const { t, errorMessage } = usePlugin()
  return (
    <>
      {error != null && (
        <p role="alert" className="break-words text-xs text-danger">
          {errorMessage(error)}
        </p>
      )}
      {running && (
        <p role="status" className="flex items-center gap-2 text-xs text-fg-muted">
          <Spinner className="size-3.5" />
          {t('loading')}
        </p>
      )}
    </>
  )
}
function Seconds({ value }: { value: number | null }) {
  const { t } = usePlugin()
  return <span className="shrink-0 font-mono text-[11px] text-fg-muted">{value === null ? t('time.missing') : t('time.value', { value })}</span>
}
export function Metrics({ report }: { report: Summary }) {
  const { t } = usePlugin()
  return (
    <div className="grid grid-cols-3 gap-2 @min-[650px]:grid-cols-6">
      {(['tests', 'passed', 'failures', 'errors', 'skipped', 'unknown'] as const).map((key) => (
        <div key={key} className="rounded-control border border-border bg-surface-1 px-3 py-2">
          <p className="text-[11px] text-fg-muted">{t(`counts.${key}`)}</p>
          <p
            className={cn(
              'text-xl font-semibold tabular-nums',
              key === 'failures' || key === 'errors' ? 'text-danger' : key === 'passed' ? 'text-success' : 'text-fg',
            )}
          >
            {report.counts[key]}
          </p>
        </div>
      ))}
    </div>
  )
}
export function Sources({
  handle,
  page,
  selected,
  onSelect,
  setPage,
}: {
  handle: string
  page: number
  selected?: number
  onSelect: (source: Source) => void
  setPage: (n: number) => void
}) {
  const { t } = usePlugin()
  const task = useRequest<Page<Source>>('browse', { handle, kind: 'sources', offset: (page - 1) * PAGE, limit: PAGE })
  return (
    <>
      <Busy error={task.error} running={task.running} />
      <div className="min-h-0 flex-1 space-y-1 overflow-auto">
        {task.result?.items.map((source) => (
          <button
            type="button"
            key={source.id}
            onClick={() => onSelect(source)}
            className={cn(
              'w-full rounded-control border p-2 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring',
              selected === source.id ? 'border-primary bg-primary-soft' : 'border-border hover:bg-hover',
            )}
          >
            <p className="truncate text-xs text-fg" title={source.path}>
              {source.name}
            </p>
            <p className="mt-1 text-[11px] text-fg-muted">{t('source.identity', { id: source.id, count: source.counts.tests })}</p>
          </button>
        ))}
      </div>
      <Pager page={page} total={task.result?.total ?? 0} setPage={setPage} />
    </>
  )
}
export function Suites({
  handle,
  source,
  parent,
  page,
  onSelect,
  setPage,
}: {
  handle: string
  source?: number
  parent?: number
  page: number
  onSelect: (s: Suite) => void
  setPage: (n: number) => void
}) {
  const { t } = usePlugin()
  const task = useRequest<Page<Suite>>('browse', { handle, kind: 'suites', filter: { source }, parent, offset: (page - 1) * PAGE, limit: PAGE })
  return (
    <>
      <Busy error={task.error} running={task.running} />
      <div className="min-h-0 flex-1 space-y-1 overflow-auto">
        {task.result?.items.map((s) => (
          <button
            key={s.id}
            type="button"
            className="flex w-full items-center gap-2 rounded-control border border-border p-2 text-left hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring"
            onClick={() => onSelect(s)}
          >
            <FolderTree className="size-3.5 shrink-0 text-fg-subtle" />
            <span className="min-w-0 flex-1">
              <span className="block truncate text-xs text-fg">{s.name || t('suite.unnamed')}</span>
              <span className="text-[11px] text-fg-muted">{t('suite.identity', { source: s.source, id: s.id, count: s.counts.tests })}</span>
            </span>
            <ChevronRight className="size-3.5 shrink-0 text-fg-subtle" />
          </button>
        ))}
        {task.result?.total === 0 && <p className="text-xs text-fg-subtle">{t('suite.noChildren')}</p>}
      </div>
      <Pager page={page} total={task.result?.total ?? 0} setPage={setPage} />
    </>
  )
}
export function Cases({
  handle,
  filter,
  page,
  selected,
  onSelect,
  setPage,
}: {
  handle: string
  filter: Filter
  page: number
  selected: Case | null
  onSelect: (c: Case) => void
  setPage: (n: number) => void
}) {
  const { t } = usePlugin()
  const task = useRequest<Page<Case>>('browse', { handle, kind: 'cases', filter, offset: (page - 1) * PAGE, limit: PAGE })
  return (
    <>
      <Busy error={task.error} running={task.running} />
      <p className="shrink-0 text-[11px] text-fg-subtle">{t('cases.total', { count: task.result?.total ?? 0 })}</p>
      <div className="min-h-0 flex-1 space-y-2 overflow-auto">
        {task.result?.items.map((c) => (
          <button
            type="button"
            key={c.id}
            onClick={() => onSelect(c)}
            className={cn(
              'w-full space-y-2 rounded-control border p-3 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring',
              selected?.id === c.id ? 'border-primary bg-primary-soft' : 'border-border hover:bg-hover',
            )}
          >
            <div className="flex items-center justify-between gap-2">
              <StatusBadge status={c.status} />
              <Seconds value={c.seconds} />
            </div>
            <p className="break-words text-[13px] font-medium text-fg">{c.name || t('cases.unnamed')}</p>
            <p className="truncate font-mono text-[11px] text-fg-muted" title={c.classname}>
              {c.classname || t('cases.noClass')}
            </p>
            <p className="text-[10px] text-fg-subtle">{t('cases.identity', { source: c.source, suite: c.suite, id: c.id })}</p>
          </button>
        ))}
        {task.result?.total === 0 && <Empty icon={<TestTube2 />} title={t('cases.empty')} />}
      </div>
      <Pager page={page} total={task.result?.total ?? 0} setPage={setPage} />
    </>
  )
}
function TextChunk({ handle, block, page, setPage }: { handle: string; block: Block; page: number; setPage: (n: number) => void }) {
  const { t } = usePlugin()
  const task = useRequest<Chunk>('content', { handle, id: block.id, offset: (page - 1) * 8192, limit: 8192 })
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2">
      <Busy error={task.error} running={task.running} />
      <div className="min-h-0 flex-1 overflow-hidden rounded-control border border-border">
        <CodeEditor value={task.result?.text ?? ''} language="text" readOnly aria-label={t('detail.text')} />
      </div>
      <p className="text-[11px] text-fg-subtle">{t('detail.characters', { count: task.result?.total ?? block.characters })}</p>
      <Pager page={page} total={task.result?.total ?? 0} size={8192} setPage={setPage} />
    </div>
  )
}
export function Details({ handle, kind, id }: { handle: string; kind: 'case' | 'suite'; id: number }) {
  const [page, setPage] = useState(1)
  const [block, setBlock] = useState<Block | null>(null)
  const [chunkPage, setChunkPage] = useState(1)
  return (
    <DetailPage
      key={`${id}:${kind}:${page}`}
      handle={handle}
      kind={kind}
      id={id}
      page={page}
      setPage={setPage}
      block={block}
      onBlock={(b) => {
        setBlock(b)
        setChunkPage(1)
      }}
      chunkPage={chunkPage}
      setChunkPage={setChunkPage}
    />
  )
}
function DetailPage({
  handle,
  kind,
  id,
  page,
  setPage,
  block,
  onBlock,
  chunkPage,
  setChunkPage,
}: {
  handle: string
  kind: 'case' | 'suite'
  id: number
  page: number
  setPage: (n: number) => void
  block: Block | null
  onBlock: (b: Block) => void
  chunkPage: number
  setChunkPage: (n: number) => void
}) {
  const { t } = usePlugin()
  const task = useRequest<Detail>('detail', { handle, id, kind, offset: (page - 1) * PAGE, limit: PAGE })
  const row = task.result?.row
  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <Busy error={task.error} running={task.running} />
      {row && (
        <div className="shrink-0 space-y-2 rounded-control border border-border bg-surface-2 p-3">
          <p className="break-words text-sm font-medium text-fg">{row.name || t('suite.unnamed')}</p>
          {'status' in row ? (
            <>
              <div className="flex items-center justify-between gap-2">
                <StatusBadge status={row.status} />
                <Seconds value={row.seconds} />
              </div>
              <p className="break-words font-mono text-[11px] text-fg-muted">{row.classname}</p>
              {row.file && (
                <p className="break-words text-xs text-fg-muted">
                  {row.file}
                  {row.line && `:${row.line}`}
                </p>
              )}
            </>
          ) : (
            <>
              <p className="text-[11px] text-fg-muted">{t('suite.declaredActual')}</p>
              <dl className="grid grid-cols-2 gap-1 text-xs">
                {(['tests', 'failures', 'errors', 'skipped'] as const).map((key) => (
                  <div key={key} className="flex gap-2">
                    <dt className="text-fg-muted">{t(`counts.${key}`)}</dt>
                    <dd className="font-mono text-fg">
                      {row.declared[key] ?? t('time.missing')} / {row.counts[key]}
                    </dd>
                  </div>
                ))}
              </dl>
              <p className="text-[11px] text-fg-subtle">
                {t('suite.times', { declared: row.declared.seconds ?? t('time.missing'), known: row.counts.knownSeconds, missing: row.counts.missingTime })}
              </p>
            </>
          )}
        </div>
      )}
      <div className="max-h-40 shrink-0 space-y-1 overflow-auto">
        {task.result?.items.map((b) => (
          <button
            type="button"
            key={b.id}
            onClick={() => onBlock(b)}
            className={cn(
              'w-full rounded-control border p-2 text-left',
              block?.id === b.id ? 'border-primary bg-primary-soft' : 'border-border hover:bg-hover',
            )}
          >
            <p className="font-mono text-xs text-fg">{b.kind}</p>
            {b.typeName && <p className="break-words text-[11px] text-fg-muted">{b.typeName}</p>}
            {b.message && <p className="break-words text-xs text-fg-muted">{b.message}</p>}
            <p className="text-[10px] text-fg-subtle">{t('detail.characters', { count: b.characters })}</p>
          </button>
        ))}
      </div>
      <Pager page={page} total={task.result?.total ?? 0} setPage={setPage} />
      {block ? (
        <TextChunk key={`${block.id}:${chunkPage}`} handle={handle} block={block} page={chunkPage} setPage={setChunkPage} />
      ) : (
        <div className="flex min-h-0 flex-1 items-center justify-center">
          <Empty icon={<FileCode2 />} title={t('detail.chooseLog')} description={t('detail.plainText')} />
        </div>
      )}
    </div>
  )
}
export function Diagnostics({ handle, page, setPage }: { handle: string; page: number; setPage: (n: number) => void }) {
  const { t } = usePlugin()
  const task = useRequest<Page<Diagnostic>>('browse', { handle, kind: 'diagnostics', offset: (page - 1) * PAGE, limit: PAGE })
  return (
    <Panel title={t('diagnostics.title')} bodyClassName="flex h-full min-h-0 flex-col gap-3 p-3">
      <Busy error={task.error} running={task.running} />
      <div className="min-h-0 flex-1 space-y-2 overflow-auto">
        {task.result?.items.map((d, index) => (
          <div key={index} className="rounded-control border border-border bg-warning-soft p-3">
            <p className="text-xs font-medium text-warning">{t(`diagnostics.${d.kind}`)}</p>
            <p className="mt-1 text-[11px] text-fg-muted">{t('cases.identity', { source: d.source, suite: d.suite ?? '—', id: d.case ?? '—' })}</p>
            {d.detail && <p className="mt-1 break-words font-mono text-xs text-fg">{d.detail}</p>}
          </div>
        ))}
        {task.result?.total === 0 && <p className="text-xs text-fg-muted">{t('diagnostics.none')}</p>}
      </div>
      <Pager page={page} total={task.result?.total ?? 0} setPage={setPage} />
    </Panel>
  )
}
