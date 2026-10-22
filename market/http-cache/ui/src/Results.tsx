import { Badge, Button, CodeEditor, Pagination, Tooltip, cn, toast } from '@toolforge/ui'
import { host, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Clock3, Copy, Database, RefreshCw } from 'lucide-react'
import type { Condition, Directive, Note, Section, Summary, Timing, VaryRow, View } from './types'
export function variant(status: string) {
  if (['allowed', 'candidate', 'fresh', 'pass', 'match'].includes(status)) return 'success' as const
  if (['blocked', 'fail', 'different'].includes(status)) return 'danger' as const
  if (['validate', 'stale', 'variant', 'unknown'].includes(status)) return 'warning' as const
  return 'neutral' as const
}
export function Cards({ summaries, selected, busy, select }: { summaries: Summary[]; selected: number; busy: boolean; select: (n: number) => void }) {
  const { t } = usePlugin()
  const seconds = (n: number | null) => (n === null ? t('common.unknown') : t('common.seconds', { n }))
  return (
    <div className={cn('grid shrink-0 gap-2', summaries.length === 2 ? 'grid-cols-2' : 'grid-cols-1')}>
      {summaries.map((s, i) => (
        <button
          key={i}
          type="button"
          disabled={busy}
          onClick={() => select(i)}
          className={cn(
            'min-w-0 space-y-2 rounded-card border p-3 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-60',
            selected === i ? 'border-primary/40 bg-primary-soft' : 'border-border bg-surface-2',
          )}
        >
          <div className="flex flex-wrap items-center justify-between gap-1">
            <h3 className="truncate text-sm font-semibold text-fg">{s.name}</h3>
            <span className="text-[11px] text-fg-muted">{t(`role.${s.role}`)}</span>
          </div>
          <Badge variant={variant(s.reuse)}>{t(`reuse.${s.reuse}`)}</Badge>
          <div className="grid grid-cols-2 gap-2 text-[11px]">
            <div>
              <span className="block text-fg-subtle">{t('result.lifetime')}</span>
              <span className="font-medium tabular-nums text-fg">{seconds(s.lifetime)}</span>
            </div>
            <div>
              <span className="block text-fg-subtle">{t('result.age')}</span>
              <span className="font-medium tabular-nums text-fg">{seconds(s.currentAge)}</span>
            </div>
          </div>
        </button>
      ))}
    </div>
  )
}
export function Explanation({ note }: { note: Note }) {
  const { t } = usePlugin()
  return (
    <span>
      {t(`messages.${note.code}`, { ...note.params, ...(typeof note.params.context === 'string' ? { context: t(`context.${note.params.context}`) } : {}) })}
    </span>
  )
}
const formulas: { key: keyof Timing; formula: string }[] = [
  { key: 'apparentAge', formula: 'max(0, response_time − Date)' },
  { key: 'responseDelay', formula: 'response_time − request_time' },
  { key: 'correctedAgeValue', formula: 'Age + response_delay' },
  { key: 'correctedInitialAge', formula: 'max(apparent_age, corrected_age_value)' },
  { key: 'residentTime', formula: 'now − response_time' },
  { key: 'currentAge', formula: 'corrected_initial_age + resident_time' },
]
export function Detail({ view, section, busy, page }: { view: View; section: Section; busy: boolean; page: (offset: number) => void }) {
  const { t, errorMessage } = usePlugin()
  const seconds = (n: number | string | null) => (n === null ? t('common.unknown') : t('common.seconds', { n }))
  const copy = async () => {
    try {
      await host.clipboard.writeText(view.conditional)
      toast.success(t('files.copied'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="min-h-0 flex-1 space-y-3 overflow-auto pr-1">
        {section === 'conditions' && (
          <>
            <div className="flex flex-wrap gap-2">
              <Badge variant={variant(view.summary.storage)}>
                <Database className="size-3" />
                {t(`storage.${view.summary.storage}`)}
              </Badge>
              <Badge variant={variant(view.summary.freshness)}>
                <Clock3 className="size-3" />
                {t(`freshness.${view.summary.freshness}`)}
              </Badge>
              <Badge variant="neutral">{t('result.source', { source: t(`source.${view.summary.lifetimeSource}`) })}</Badge>
            </div>
            {(view.page.items as Condition[]).map((c) => (
              <div key={c.key} className="flex items-start gap-2 rounded-control border border-border p-3">
                <Badge variant={variant(c.status)}>{t(`conditionStatus.${c.status}`)}</Badge>
                <p className="min-w-0 text-xs leading-relaxed text-fg-muted">
                  <Explanation note={c.note} />
                </p>
              </div>
            ))}
            <div className="space-y-2 rounded-card border border-border bg-surface-2 p-3">
              <div className="flex items-center justify-between gap-2">
                <h3 className="flex items-center gap-2 text-sm font-medium text-fg">
                  <RefreshCw className="size-4" />
                  {t('result.conditional')}
                </h3>
                <Tooltip content={t('result.copyConditional')}>
                  <Button size="icon-sm" variant="ghost" disabled={!view.conditional} aria-label={t('result.copyConditional')} onClick={() => void copy()}>
                    <Copy />
                  </Button>
                </Tooltip>
              </div>
              <p className="text-xs leading-relaxed text-fg-muted">{t('result.conditionalHint')}</p>
              {view.conditional ? (
                <pre className="overflow-x-auto rounded-control bg-surface p-2 font-mono text-xs text-fg">{view.conditional}</pre>
              ) : (
                <p className="text-xs text-fg-subtle">{t('result.noValidators')}</p>
              )}
            </div>
          </>
        )}
        {section === 'timing' && (
          <>
            <div className="grid grid-cols-2 gap-2 rounded-card border border-border bg-surface-2 p-3">
              <div>
                <p className="text-xs text-fg-subtle">{t('result.remaining')}</p>
                <p className="mt-1 text-lg font-semibold tabular-nums text-fg">{seconds(view.timing.remaining)}</p>
              </div>
              <div>
                <p className="text-xs text-fg-subtle">{t('result.rawAge')}</p>
                <p className="mt-1 text-lg font-semibold tabular-nums text-fg">{seconds(view.timing.ageValue)}</p>
              </div>
            </div>
            <p className="text-xs text-fg-muted">{t('result.ageHint')}</p>
            {formulas.map((row) => (
              <div key={row.key} className="rounded-control border border-border p-3">
                <div className="mb-1 flex items-center justify-between gap-2">
                  <h3 className="text-xs font-medium text-fg">{t(`timing.${row.key}`)}</h3>
                  <span className="text-sm font-semibold tabular-nums text-fg">{seconds(view.timing[row.key])}</span>
                </div>
                <code className="block overflow-x-auto text-[11px] text-fg-subtle">{row.formula}</code>
              </div>
            ))}
          </>
        )}
        {section === 'vary' && (
          <>
            <p className="text-xs leading-relaxed text-fg-muted">{t('result.varyHint')}</p>
            {(view.page.items as VaryRow[]).map((row) => (
              <div key={row.name} className="space-y-2 rounded-card border border-border p-3">
                <div className="flex items-center justify-between gap-2">
                  <code className="truncate text-xs font-medium text-fg">{row.name}</code>
                  <Badge variant={variant(row.status)}>{t(`vary.${row.status}`)}</Badge>
                </div>
                <div className="grid grid-cols-2 gap-3">
                  {(['stored', 'presented'] as const).map((key) => (
                    <div key={key} className="min-w-0">
                      <p className="mb-1 text-[11px] text-fg-subtle">{t(`vary.${key}`)}</p>
                      <p className="font-mono text-xs break-all text-fg">
                        {row[key] === null ? t('vary.absent') : row[key] === '' ? t('vary.empty') : row[key]}
                      </p>
                    </div>
                  ))}
                </div>
              </div>
            ))}
            {view.page.total === 0 && <p className="p-4 text-center text-xs text-fg-subtle">{t('result.noVary')}</p>}
          </>
        )}
        {section === 'directives' &&
          (view.page.items as Directive[]).map((d, i) => (
            <div key={i} className="space-y-2 rounded-control border border-border p-3">
              <div className="flex items-center gap-2">
                <Badge variant="neutral">{t(`context.${d.context}`)}</Badge>
                <Badge variant={d.valid && d.supported ? 'success' : 'warning'}>
                  {t(d.valid ? (d.supported ? 'directives.supported' : 'directives.boundary') : 'directives.invalid')}
                </Badge>
              </div>
              <code className="block text-xs break-all text-fg">
                {d.name}
                {d.value === null ? '' : `=${d.value}`}
              </code>
            </div>
          ))}
        {section === 'diagnostics' && (
          <>
            {(view.page.items as Note[]).map((note, i) => (
              <p key={i} className="rounded-control border border-warning/20 bg-warning/5 p-3 text-xs leading-relaxed text-fg-muted">
                <Explanation note={note} />
              </p>
            ))}
            {view.page.total === 0 && <p className="p-4 text-center text-xs text-fg-subtle">{t('result.noDiagnostics')}</p>}
          </>
        )}
        {section === 'headers' && (
          <>
            <p className="text-xs text-fg-muted">{t('result.maskHint')}</p>
            {(['request', 'response', 'presented'] as const).map((key) => (
              <div key={key} className="space-y-2">
                <h3 className="text-xs font-medium text-fg">{t(`context.${key}`)}</h3>
                <div className="h-48 overflow-hidden rounded-control border border-border">
                  <CodeEditor
                    value={view.headers[key].map((h) => `${h.name}: ${h.value}`).join('\n')}
                    readOnly
                    language="text"
                    aria-label={t(`context.${key}`)}
                  />
                </div>
              </div>
            ))}
          </>
        )}
      </div>
      {section !== 'timing' && section !== 'headers' && (
        <div className={busy ? 'pointer-events-none opacity-50' : ''}>
          <Pagination
            page={view.page.offset / view.page.limit + 1}
            pageSize={view.page.limit}
            total={view.page.total}
            onPageChange={(n) => {
              if (!busy) page((n - 1) * view.page.limit)
            }}
            prevLabel={t('page.prev')}
            nextLabel={t('page.next')}
            pageLabel={(n) => t('page.number', { n })}
            totalLabel={t('page.total', { n: view.page.total })}
          />
        </div>
      )}
    </div>
  )
}
