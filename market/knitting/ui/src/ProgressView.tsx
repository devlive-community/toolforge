import { Check, Minus, Plus, RotateCcw, SkipForward } from 'lucide-react'
import { Badge, Button, Empty, Pagination, Progress, Tooltip } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import type { Page, Pages, PartView, Op } from './types'

export function PageBar<T>({ page, change }: { page: Page<T>; change: (offset: number) => void }) {
  const { t } = usePlugin()
  return <Pagination page={Math.floor(page.offset / page.limit) + 1} pageSize={page.limit} total={page.total} onPageChange={(p) => change((p - 1) * page.limit)} prevLabel={t('page.previous')} nextLabel={t('page.next')} pageLabel={(p) => t('page.number', { page: p })} totalLabel={t('page.total', { count: page.total })} />
}

export function ProgressView({ part, busy, action, page }: { part: PartView; busy: boolean; action: (op: Op) => void; page: (key: keyof Pages, offset: number) => void }) {
  const { t } = usePlugin()
  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="min-h-0 flex-1 space-y-3 overflow-auto pr-1">
      <div className="grid shrink-0 grid-cols-2 gap-3">
        <div className="rounded-card border border-border bg-surface-2 p-4"><p className="text-xs text-fg-muted">{t(`counter.completed.${part.kind}`)}</p><p className="mt-2 font-mono text-4xl font-semibold tracking-tight text-fg">{part.completed}</p></div>
        <div className="rounded-card border border-border bg-surface-2 p-4"><p className="text-xs text-fg-muted">{t('counter.stitches')}</p><p className="mt-2 font-mono text-4xl font-semibold tracking-tight text-fg">{part.stitches}</p><p className="mt-1 text-[11px] text-fg-subtle">{t('counter.initial', { value: part.initialStitches })}</p></div>
      </div>
      <div className="shrink-0 rounded-card border border-primary/20 bg-primary-soft p-3.5">
        <div className="flex items-center justify-between gap-2"><h3 className="text-base font-semibold text-fg">{t(`counter.next.${part.kind}`, { line: part.nextLine })}</h3><Badge variant="neutral">{t(`mode.${part.kind}`)}</Badge></div>
        {part.pattern ? <><p className="mt-2 text-sm text-fg">{part.pattern.name || t('segment.name', { index: part.pattern.index })}</p><p className="mt-1 text-xs text-fg-muted">{t('segment.position', { unit: t(`one.${part.kind}`), row: part.pattern.row, rows: part.pattern.rows, repeat: part.pattern.repeat, repeats: part.pattern.repeats })}</p><Progress value={part.pattern.completedRepeats / part.pattern.repeats * 100} className="mt-2" /><p className="mt-1 text-[11px] text-fg-subtle">{t('segment.completed', { value: part.pattern.completedRepeats })}</p></> : <p className="mt-2 text-xs text-fg-muted">{t(part.patternFinished ? 'segment.finished' : 'segment.free')}</p>}
      </div>
      <div className="space-y-2">
        <h4 className="text-xs font-semibold text-fg">{t('reminder.forNext', { count: part.dueCount })}</h4>
        {part.dueCount === 0 ? <p className="rounded-control border border-border p-3 text-xs text-fg-muted">{t('reminder.none')}</p> : part.due.items.map((r) => <div key={r.id} className="rounded-control border border-border p-3">
          <div className="flex items-center gap-2"><span className="min-w-0 flex-1 truncate text-sm font-medium text-fg">{r.name || t('reminder.name', { index: r.index })}</span><Badge variant={r.delta > 0 ? 'success' : 'warning'}>{r.delta > 0 ? '+' : ''}{r.delta}</Badge></div>
          <p className="mt-1 text-[11px] text-fg-subtle">{t('reminder.occurrence', { value: r.occurrence, total: r.repeats })}</p>
          <div className="mt-2 flex gap-2"><Button size="sm" disabled={busy} aria-pressed={r.choice === 'execute'} variant={r.choice === 'execute' ? 'primary' : 'outline'} onClick={() => action({ type: 'choose', id: r.id, choice: r.choice === 'execute' ? null : 'execute' })}><Check />{t('reminder.execute')}</Button><Button size="sm" disabled={busy} aria-pressed={r.choice === 'skip'} variant={r.choice === 'skip' ? 'secondary' : 'ghost'} onClick={() => action({ type: 'choose', id: r.id, choice: r.choice === 'skip' ? null : 'skip' })}><SkipForward />{t('reminder.skip')}</Button></div>
        </div>)}
        <PageBar page={part.due} change={(offset) => page('due', offset)} />
        {part.dueCount > 0 && <p className="text-[11px] leading-relaxed text-fg-subtle">{t('reminder.notActual')}</p>}
        {part.note && <p className="whitespace-pre-wrap rounded-control bg-surface-2 p-3 text-xs leading-relaxed text-fg-muted">{part.note}</p>}
      </div>
      </div>
      <div className="shrink-0 space-y-2 border-t border-border pt-3">
        {part.pendingCount > 0 && <p className="text-xs text-warning">{t('reminder.pending', { count: part.pendingCount })}</p>}
        {part.pendingCount === 0 && part.projectedStitches === null && <p className="text-xs text-danger">{t('reminder.invalidStitches')}</p>}
        {part.pendingCount === 0 && part.dueCount > 0 && part.projectedStitches !== null && <p className="text-xs text-fg-muted">{t('reminder.projected', { value: part.projectedStitches })}</p>}
        <Button block variant="primary" className="h-12 text-base" disabled={busy || !part.canComplete} onClick={() => action({ type: 'complete', expectedLine: part.nextLine })}><Check />{t(`counter.complete.${part.kind}`)}</Button><div className="flex items-center justify-between gap-3"><Button size="sm" variant="ghost" disabled={busy || !part.canUndo} onClick={() => action({ type: 'undo' })}><RotateCcw />{t('counter.undo')}</Button><p className="text-[11px] text-fg-subtle">{t('counter.shortcut')}</p></div></div>
    </div>
  )
}

export function RulesView({ part, busy, page, remove, add }: { part: PartView; busy: boolean; page: (key: keyof Pages, offset: number) => void; remove: (op: Op, kind: string) => void; add: (kind: 'segment' | 'reminder') => void }) {
  const { t } = usePlugin()
  return <div className="h-full space-y-4 overflow-auto pr-1">
    <section className="space-y-2"><div className="flex items-center justify-between"><h4 className="text-sm font-semibold text-fg">{t('segment.title')}</h4><Button size="sm" variant="soft" disabled={busy || part.segments.total >= 20} onClick={() => add('segment')}><Plus />{t('segment.add')}</Button></div>
      {part.segments.items.length === 0 && <p className="py-2 text-xs text-fg-muted">{t('segment.empty')}</p>}
      {part.segments.items.map((s) => <div key={s.id} className="rounded-control border border-border p-3"><div className="flex items-center justify-between gap-2"><span className="text-sm font-medium text-fg">{s.name || t('segment.name', { index: s.index })}</span><Tooltip content={t('segment.remove')}><Button size="icon-sm" variant="ghost" disabled={busy || !part.canEditRules} aria-label={t('segment.remove')} onClick={() => remove({ type: 'removeSegment', id: s.id }, 'segment')}><Minus /></Button></Tooltip></div><p className="mt-1 text-xs text-fg-muted">{t('segment.range', { start: s.start, end: s.end, unit: t(`one.${part.kind}`), rows: s.rows, repeats: s.repeats })}</p><p className="mt-1 text-[11px] text-fg-subtle">{t('segment.completed', { value: s.completedRepeats })}</p></div>)}
      <PageBar page={part.segments} change={(offset) => page('segments', offset)} />
    </section>
    <section className="space-y-2"><div className="flex items-center justify-between"><h4 className="text-sm font-semibold text-fg">{t('reminder.title')}</h4><Button size="sm" variant="soft" disabled={busy || part.reminders.total >= 20} onClick={() => add('reminder')}><Plus />{t('reminder.add')}</Button></div>
      {part.reminders.items.length === 0 && <p className="py-2 text-xs text-fg-muted">{t('reminder.empty')}</p>}
      {part.reminders.items.map((r) => <div key={r.id} className="rounded-control border border-border p-3"><div className="flex items-center gap-2"><span className="min-w-0 flex-1 text-sm font-medium text-fg">{r.name || t('reminder.name', { index: r.index })}</span><Badge variant={r.delta > 0 ? 'success' : 'warning'}>{r.delta > 0 ? '+' : ''}{r.delta}</Badge><Tooltip content={t('reminder.remove')}><Button size="icon-sm" variant="ghost" disabled={busy || !part.canEditRules} aria-label={t('reminder.remove')} onClick={() => remove({ type: 'removeReminder', id: r.id }, 'reminder')}><Minus /></Button></Tooltip></div><p className="mt-1 text-xs text-fg-muted">{t('reminder.schedule', { start: r.start, interval: r.interval, repeats: r.repeats, unit: t(`one.${part.kind}`) })}</p><p className="mt-1 text-[11px] text-fg-subtle">{t('reminder.actual', { executed: r.executed, skipped: r.skipped })}</p><p className="mt-1 text-[11px] text-fg-subtle">{r.next === null ? t('reminder.finished') : t('reminder.next', { value: r.next, unit: t(`one.${part.kind}`) })}</p></div>)}
      <PageBar page={part.reminders} change={(offset) => page('reminders', offset)} />
    </section>
    {!part.canEditRules && <p className="rounded-control bg-surface-2 p-2.5 text-xs leading-relaxed text-fg-muted">{t('rules.locked')}</p>}
  </div>
}

export function HistoryView({ part, page }: { part: PartView; page: (key: keyof Pages, offset: number) => void }) {
  const { t } = usePlugin()
  if (part.history.total === 0) return <Empty icon={<RotateCcw />} title={t('history.empty')} />
  return <div className="flex h-full min-h-0 flex-col gap-3"><div className="min-h-0 flex-1 space-y-2 overflow-auto">{part.history.items.map((h) => <div key={h.index} className="rounded-control border border-border p-3"><div className="flex items-center justify-between text-sm text-fg"><span>{t(`history.${h.kind}`)}</span><span className="font-mono text-xs text-fg-subtle">{h.index}</span></div><p className="mt-1 text-xs text-fg-muted">{t('history.completed', { before: h.completedBefore, after: h.completedAfter, unit: t(`one.${part.kind}`) })}</p><p className="mt-1 text-xs text-fg-muted">{t('history.stitches', { before: h.stitchesBefore, after: h.stitchesAfter })}</p></div>)}</div><PageBar page={part.history} change={(offset) => page('history', offset)} /></div>
}
