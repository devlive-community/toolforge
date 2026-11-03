import { useEffect, useRef, useState } from 'react'
import { host, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { Badge, Button, Empty, Pagination, Spinner, cn } from '@toolforge/ui'
import { ExternalLink, FolderOpen, ShieldQuestion } from 'lucide-react'
import type {
  ArtifactRow,
  Counts,
  Detail,
  Diagnostic,
  Filter,
  Level,
  LocationRow,
  Page,
  ResultRow,
  RuleRow,
  RunRow,
  SourceChunk,
} from './types'

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
export function LevelBadge({ level }: { level: Level }) {
  const { t } = usePlugin()
  return (
    <Badge variant={level === 'error' ? 'danger' : level === 'warning' ? 'warning' : level === 'note' ? 'info' : 'neutral'}>
      {t(`level.${level}`)}
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
function ListFrame({ children, page, total, setPage }: { children: React.ReactNode; page: number; total: number; setPage: (n: number) => void }) {
  return (
    <>
      <div className="min-h-0 flex-1 space-y-1 overflow-auto">{children}</div>
      <Pager page={page} total={total} setPage={setPage} />
    </>
  )
}
function pickStyle(selected: boolean) {
  return selected ? 'border-primary bg-primary-soft' : 'border-border hover:bg-hover'
}
export function Runs({ handle, selected, onSelect }: { handle: string; selected?: number; onSelect: (run: RunRow | null) => void }) {
  const { t } = usePlugin()
  const [page, setPage] = useState(1)
  const task = useRequest<Page<RunRow>>('browse', { handle, kind: 'runs', offset: (page - 1) * PAGE, limit: PAGE })
  return (
    <ListFrame page={page} total={task.result?.total ?? 0} setPage={setPage}>
      {task.result?.items.map((run) => (
        <button
          type="button"
          key={run.id}
          onClick={() => onSelect(run.id === selected ? null : run)}
          className={cn('w-full rounded-control border p-2 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring', pickStyle(selected === run.id))}
        >
          <p className="truncate text-xs text-fg" title={run.tool}>
            {run.tool}
            {run.version && <span className="text-fg-muted"> {run.version}</span>}
          </p>
          <p className="mt-1 text-[11px] text-fg-muted">{t('run.identity', { id: run.id, count: run.resultCount })}</p>
        </button>
      ))}
    </ListFrame>
  )
}
export function Rules({ handle, run, selected, onSelect }: { handle: string; run?: number; selected?: string; onSelect: (rule: RuleRow | null) => void }) {
  const { t } = usePlugin()
  const [page, setPage] = useState(1)
  const task = useRequest<Page<RuleRow>>('browse', { handle, kind: 'rules', filter: { run }, offset: (page - 1) * PAGE, limit: PAGE })
  return (
    <ListFrame page={page} total={task.result?.total ?? 0} setPage={setPage}>
      {task.result?.items.map((rule) => (
        <button
          type="button"
          key={rule.id}
          onClick={() => onSelect(rule.ruleId === selected ? null : rule)}
          className={cn('w-full rounded-control border p-2 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring', pickStyle(selected === rule.ruleId))}
        >
          <p className="truncate font-mono text-xs text-fg" title={rule.ruleId}>
            {rule.ruleId}
          </p>
          <p className="mt-1 truncate text-[11px] text-fg-muted">
            {rule.shortDescription || t('rule.noDescription')}
            {rule.defaultLevel && ` · ${t(`level.${rule.defaultLevel}`)}`}
          </p>
        </button>
      ))}
    </ListFrame>
  )
}
export function Artifacts({ handle, run, selected, onSelect }: { handle: string; run?: number; selected?: number; onSelect: (artifact: ArtifactRow | null) => void }) {
  const { t } = usePlugin()
  const [page, setPage] = useState(1)
  const task = useRequest<Page<ArtifactRow>>('browse', { handle, kind: 'artifacts', filter: { run }, offset: (page - 1) * PAGE, limit: PAGE })
  return (
    <ListFrame page={page} total={task.result?.total ?? 0} setPage={setPage}>
      {task.result?.items.map((artifact) => (
        <button
          type="button"
          key={artifact.id}
          onClick={() => onSelect(artifact.id === selected ? null : artifact)}
          className={cn('w-full rounded-control border p-2 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring', pickStyle(selected === artifact.id))}
        >
          <p className="truncate font-mono text-xs text-fg" title={artifact.uri}>
            {artifact.uri || t('artifact.noUri')}
          </p>
          <p className="mt-1 text-[11px] text-fg-muted">{t('artifact.identity', { id: artifact.id, count: artifact.roles.length })}</p>
        </button>
      ))}
    </ListFrame>
  )
}
export function Results({
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
  selected: ResultRow | null
  onSelect: (row: ResultRow) => void
  setPage: (n: number) => void
}) {
  const { t } = usePlugin()
  const task = useRequest<Page<ResultRow>>('browse', { handle, kind: 'results', filter, offset: (page - 1) * PAGE, limit: PAGE })
  return (
    <>
      <Busy error={task.error} running={task.running} />
      <p className="shrink-0 text-[11px] text-fg-subtle">{t('results.total', { count: task.result?.total ?? 0 })}</p>
      <ListFrame page={page} total={task.result?.total ?? 0} setPage={setPage}>
        {task.result?.items.map((row) => (
          <button
            type="button"
            key={row.id}
            onClick={() => onSelect(row)}
            className={cn('w-full space-y-1 rounded-control border p-3 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring', pickStyle(selected?.id === row.id))}
          >
            <div className="flex flex-wrap items-center gap-2">
              <LevelBadge level={row.level} />
              <span className="font-mono text-[11px] text-fg-muted">{row.ruleId}</span>
              {row.suppressed && <Badge variant="neutral">{t('suppression.badge')}</Badge>}
              {row.baselineState && <Badge variant="info">{t(`baseline.${row.baselineState}`, { defaultValue: row.baselineState })}</Badge>}
              <span className="flex-1" />
              <span className="text-[10px] text-fg-subtle">#{row.id}</span>
            </div>
            <p className="break-words text-[13px] text-fg">{row.message || t('results.noMessage')}</p>
            <p className="text-[10px] text-fg-subtle">{t('results.identity', { run: row.run, id: row.id, kind: row.kind })}</p>
          </button>
        ))}
        {task.result?.total === 0 && <Empty icon={<ShieldQuestion />} title={t('results.empty')} description={t('results.emptyHint')} />}
      </ListFrame>
    </>
  )
}
/** Column highlight honours the run's declared columnKind. */
function Marked({ text, kind, startColumn, endColumn }: { text: string; kind: string; startColumn?: number | null; endColumn?: number | null }) {
  if (!startColumn) return <>{text}</>
  const from = startColumn - 1
  const to = Math.max(endColumn ?? startColumn, startColumn)
  if (kind === 'unicodeCodePoints') {
    const chars = Array.from(text)
    return (
      <>
        {chars.slice(0, from).join('')}
        <span className="rounded-sm bg-primary-soft">{chars.slice(from, to).join('')}</span>
        {chars.slice(to).join('')}
      </>
    )
  }
  return (
    <>
      {text.slice(0, from)}
      <span className="rounded-sm bg-primary-soft">{text.slice(from, to)}</span>
      {text.slice(to)}
    </>
  )
}
function SourceContext({ handle, location, columnKind, region }: { handle: string; location: number; columnKind: string; region: LocationRow['region'] }) {
  const { t, errorMessage } = usePlugin()
  const [root, setRoot] = useState<string | null>(null)
  const [pickError, setPickError] = useState<unknown>(null)
  const task = useTask<SourceChunk>()
  const { start, cancel } = task
  const pick = async () => {
    try {
      setPickError(null)
      setRoot(await host.dialog.openDirectory())
    } catch (error) {
      setPickError(error)
    }
  }
  useEffect(() => {
    if (!root) return
    void start('readSource', { handle, location, root, context: 4 })
    return () => {
      void cancel()
    }
  }, [handle, location, root, start, cancel])
  if (!root) {
    return (
      <div className="rounded-control border border-dashed border-border p-3">
        {pickError != null && (
          <p role="alert" className="mb-2 text-xs text-danger">
            {errorMessage(pickError)}
          </p>
        )}
        <p className="text-xs text-fg-muted">{t('source.hint')}</p>
        <Button size="sm" variant="secondary" className="mt-2" onClick={() => void pick()}>
          <FolderOpen />
          {t('source.link')}
        </Button>
      </div>
    )
  }
  const chunk = task.result
  return (
    <div className="min-h-0 flex-1 space-y-2">
      <Busy error={task.error} running={task.running} />
      {chunk && (
        <>
          <p className="break-all font-mono text-[11px] text-fg-subtle" title={chunk.resolved}>
            {chunk.resolved}
          </p>
          <div className="max-h-64 overflow-auto rounded-control border border-border bg-surface-1 p-2 font-mono text-[11px] leading-5">
            {chunk.lines.map((line) => {
              const inside =
                region != null &&
                line.no >= (region.startLine ?? 0) &&
                line.no <= (region.endLine ?? region.startLine ?? 0)
              return (
                <div key={line.no} className={cn('flex gap-3 px-1', inside && 'bg-primary-soft')}>
                  <span className="w-10 shrink-0 select-none text-right text-fg-subtle">{line.no}</span>
                  <span className="whitespace-pre-wrap break-all text-fg">
                    {inside ? <Marked text={line.text} kind={columnKind} startColumn={region?.startColumn} endColumn={region?.endColumn} /> : line.text}
                  </span>
                </div>
              )
            })}
          </div>
          {chunk.truncated && <p className="text-[11px] text-warning">{t('source.truncated')}</p>}
          <p className="text-[11px] text-fg-subtle">
            {t('source.columns', { kind: chunk.columnKind })}
            {chunk.notes.length > 0 && ` · ${chunk.notes.map((note) => t(`source.note.${note}`, { defaultValue: note })).join(' · ')}`}
          </p>
        </>
      )}
      {task.error != null && <p className="text-xs text-danger">{errorMessage(task.error)}</p>}
    </div>
  )
}
function LocationCard({ handle, location, columnKind }: { handle: string; location: LocationRow; columnKind: string }) {
  const { t } = usePlugin()
  const [open, setOpen] = useState(false)
  const where = location.uri || location.logical
  const position = location.region?.startLine != null ? `:${location.region.startLine}` : ''
  return (
    <div className="space-y-2 rounded-control border border-border p-2">
      <div className="flex flex-wrap items-center gap-2">
        <Badge variant="neutral">{t(`location.${location.kind}`)}</Badge>
        <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-fg-muted" title={where ?? undefined}>
          {where}
          {position}
        </span>
        {location.uri && location.region?.startLine != null && (
          <Button size="sm" variant="ghost" onClick={() => setOpen(!open)}>
            <ExternalLink />
            {open ? t('source.hide') : t('source.show')}
          </Button>
        )}
      </div>
      {location.message && <p className="break-words text-xs text-fg-muted">{location.message}</p>}
      {location.snippet && <pre className="max-h-32 overflow-auto rounded-control bg-surface-1 p-2 font-mono text-[11px] text-fg">{location.snippet}</pre>}
      {open && location.uri && <SourceContext handle={handle} location={location.id} columnKind={columnKind} region={location.region} />}
    </div>
  )
}
export function ResultDetail({ handle, id }: { handle: string; id: number }) {
  const { t } = usePlugin()
  const task = useRequest<Detail>('detail', { handle, id })
  const detail = task.result
  const columnKind = detail?.columnKind ?? 'utf16CodeUnits'
  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <Busy error={task.error} running={task.running} />
      {detail && (
        <>
          <div className="shrink-0 space-y-2 rounded-control border border-border bg-surface-2 p-3">
            <div className="flex flex-wrap items-center gap-2">
              <LevelBadge level={detail.row.level} />
              <span className="font-mono text-[11px] text-fg-muted">{detail.row.ruleId}</span>
              <span className="flex-1" />
              {detail.row.occurrenceCount != null && <span className="text-[11px] text-fg-subtle">{t('detail.occurrences', { count: detail.row.occurrenceCount })}</span>}
            </div>
            <p className="break-words text-sm text-fg">{detail.row.message || t('results.noMessage')}</p>
            <dl className="grid grid-cols-2 gap-1 text-xs">
              <div className="flex gap-2">
                <dt className="text-fg-muted">{t('detail.kind')}</dt>
                <dd className="font-mono text-fg">{detail.row.kind}</dd>
              </div>
              <div className="flex gap-2">
                <dt className="text-fg-muted">{t('detail.baseline')}</dt>
                <dd className="font-mono text-fg">{detail.row.baselineState ?? t('detail.missing')}</dd>
              </div>
              {detail.row.rank != null && (
                <div className="flex gap-2">
                  <dt className="text-fg-muted">{t('detail.rank')}</dt>
                  <dd className="font-mono text-fg">{detail.row.rank}</dd>
                </div>
              )}
              <div className="flex gap-2">
                <dt className="text-fg-muted">{t('detail.suppressed')}</dt>
                <dd className="font-mono text-fg">{detail.row.suppressed ? t('suppression.badge') : t('detail.no')}</dd>
              </div>
            </dl>
            {detail.rule?.helpUri && (
              <a href={detail.rule.helpUri} target="_blank" rel="noreferrer" className="inline-flex items-center gap-1 text-xs text-primary hover:underline">
                {detail.rule.helpUri}
                <ExternalLink className="size-3" />
              </a>
            )}
          </div>
          {detail.row.suppressions.length > 0 && (
            <div className="shrink-0 space-y-1">
              {detail.row.suppressions.map((suppression, index) => (
                <p key={index} className="text-[11px] text-fg-muted">
                  {t('suppression.entry', { kind: suppression.kind, status: suppression.status ?? '—', justification: suppression.justification ? ` · ${suppression.justification}` : '' })}
                </p>
              ))}
            </div>
          )}
          {detail.rule?.shortDescription && <p className="shrink-0 text-xs text-fg-muted">{detail.rule.shortDescription}</p>}
          <div className="min-h-0 flex-1 space-y-2 overflow-auto">
            <p className="shrink-0 text-[11px] font-medium text-fg-muted">{t('detail.locations')}</p>
            {detail.locations.map((location) => (
              <LocationCard key={location.id} handle={handle} location={location} columnKind={columnKind} />
            ))}
            {detail.related.length > 0 && (
              <>
                <p className="shrink-0 pt-1 text-[11px] font-medium text-fg-muted">{t('detail.related')}</p>
                {detail.related.map((location) => (
                  <LocationCard key={location.id} handle={handle} location={location} columnKind={columnKind} />
                ))}
              </>
            )}
            {detail.flows.length > 0 && (
              <>
                <p className="shrink-0 pt-1 text-[11px] font-medium text-fg-muted">{t('detail.flows')}</p>
                {detail.flows.map((flow) => (
                  <div key={flow.id} className="space-y-1 rounded-control border border-border p-2">
                    {flow.message && <p className="text-[11px] text-fg-muted">{flow.message}</p>}
                    <ol className="space-y-1">
                      {flow.steps.map((step) => (
                        <li key={step.id} className="flex gap-2 text-[11px]">
                          <span className="w-6 shrink-0 text-right text-fg-subtle">{step.order ?? '·'}</span>
                          <span className="min-w-0 flex-1 break-words text-fg">
                            {step.message || step.logical || step.uri}
                            {step.region?.startLine != null && <span className="text-fg-subtle"> :{step.region.startLine}</span>}
                          </span>
                        </li>
                      ))}
                    </ol>
                  </div>
                ))}
              </>
            )}
            {detail.diagnostics.length > 0 && (
              <>
                <p className="shrink-0 pt-1 text-[11px] font-medium text-fg-muted">{t('detail.diagnostics')}</p>
                {detail.diagnostics.map((diagnostic, index) => (
                  <p key={index} className="break-words text-[11px] text-warning">
                    {t(`diagnostic.${diagnostic.kind}`, { defaultValue: diagnostic.kind })} {diagnostic.detail}
                  </p>
                ))}
              </>
            )}
          </div>
        </>
      )}
    </div>
  )
}
export function Diagnostics({ handle, run, page, setPage }: { handle: string; run?: number; page: number; setPage: (n: number) => void }) {
  const { t } = usePlugin()
  const task = useRequest<Page<Diagnostic>>('browse', { handle, kind: 'diagnostics', filter: { run }, offset: (page - 1) * PAGE, limit: PAGE })
  return (
    <>
      <Busy error={task.error} running={task.running} />
      {task.result?.total === 0 && <Empty icon={<ShieldQuestion />} title={t('diagnostics.none')} />}
      <ListFrame page={page} total={task.result?.total ?? 0} setPage={setPage}>
        {task.result?.items.map((diagnostic, index) => (
          <div key={index} className="rounded-control border border-border p-2">
            <p className="text-[11px] font-medium text-warning">{t(`diagnostic.${diagnostic.kind}`, { defaultValue: diagnostic.kind })}</p>
            <p className="break-words text-[11px] text-fg-muted">
              {t('diagnostics.identity', { run: diagnostic.run, result: diagnostic.result ?? '—' })} {diagnostic.detail}
            </p>
          </div>
        ))}
      </ListFrame>
    </>
  )
}
export function CountsStrip({ counts }: { counts: Counts }) {
  const { t } = usePlugin()
  return (
    <div className="grid grid-cols-3 gap-2 @min-[650px]:grid-cols-6">
      {(['results', 'errors', 'warnings', 'notes', 'none', 'suppressed'] as const).map((key) => (
        <div key={key} className="rounded-control border border-border bg-surface-1 px-3 py-2">
          <p className="text-[11px] text-fg-muted">{t(`counts.${key}`)}</p>
          <p className={cn('text-xl font-semibold tabular-nums', key === 'errors' ? 'text-danger' : key === 'warnings' ? 'text-warning' : 'text-fg')}>{counts[key]}</p>
        </div>
      ))}
    </div>
  )
}
