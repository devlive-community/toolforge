import { Badge, Button, Checkbox, Empty, Pagination, Tooltip, cn } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { Camera, Check, Clock3, Leaf, Pause, Pencil, Play, RotateCcw, Star, Trash2 } from 'lucide-react'
import type { Care, Editor, Entry, Op, Page, View } from './types'
export function PageBar({ page, change, busy }: { page: Page<unknown>; change: (offset: number) => void; busy: boolean }) {
  const { t } = usePlugin()
  return (
    <div className={busy ? 'pointer-events-none opacity-50' : ''}>
      <Pagination
        page={page.offset / page.limit + 1}
        pageSize={page.limit}
        total={page.total}
        onPageChange={(p) => {
          if (!busy) change((p - 1) * page.limit)
        }}
        prevLabel={t('page.prev')}
        nextLabel={t('page.next')}
        pageLabel={(n) => t('page.number', { n })}
        totalLabel={t('page.total', { n: page.total })}
      />
    </div>
  )
}
export function CareList({
  view,
  busy,
  edit,
  action,
  change,
}: {
  view: View
  busy: boolean
  edit: (editor: Editor) => void
  action: (op: Op) => void
  change: (offset: number) => void
}) {
  const { t } = usePlugin()
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="min-h-0 flex-1 space-y-3 overflow-auto pr-1">
        {(view.page.items as Care[]).map((care) => (
          <article
            key={care.id}
            className={cn('space-y-3 rounded-card border p-3', care.status === 'overdue' ? 'border-warning/30 bg-warning/5' : 'border-border bg-surface-2')}
          >
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <p className="text-[11px] text-fg-subtle">{care.plantName}</p>
                <h3 className="mt-1 truncate text-sm font-semibold text-fg">{care.name}</h3>
              </div>
              <Badge variant={care.status === 'overdue' ? 'warning' : care.status === 'today' ? 'primary' : 'neutral'}>{t(`status.${care.status}`)}</Badge>
            </div>
            <div className="flex flex-wrap gap-x-5 gap-y-1 text-xs text-fg-muted">
              <span>{t('care.due', { date: care.due })}</span>
              <span>{t('care.interval', { n: care.interval })}</span>
              <span>{t('care.last', { date: care.last ?? t('care.never') })}</span>
              {care.postponed && <span>{t('care.original', { date: care.baseDue })}</span>}
            </div>
            <div className="flex flex-wrap items-center gap-1.5">
              <Button size="sm" variant="primary" disabled={busy || care.paused || care.archived} onClick={() => edit({ kind: 'complete', care })}>
                <Check />
                {t('care.complete')}
              </Button>
              <Button size="sm" variant="ghost" disabled={busy || care.paused || care.archived} onClick={() => edit({ kind: 'postpone', care })}>
                <Clock3 />
                {t('care.postpone')}
              </Button>
              <Tooltip content={t('care.edit')}>
                <Button variant="ghost" size="icon-sm" disabled={busy} aria-label={t('care.edit')} onClick={() => edit({ kind: 'editPlan', care })}>
                  <Pencil />
                </Button>
              </Tooltip>
              <Tooltip content={t(care.paused ? 'care.resume' : 'care.pause')}>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  disabled={busy}
                  aria-label={t(care.paused ? 'care.resume' : 'care.pause')}
                  onClick={() => action({ type: 'pause', plant: care.plantId, plan: care.id, paused: !care.paused })}
                >
                  {care.paused ? <Play /> : <Pause />}
                </Button>
              </Tooltip>
              {care.postponed && (
                <Button size="sm" variant="ghost" disabled={busy} onClick={() => action({ type: 'postpone', plant: care.plantId, plan: care.id, date: null })}>
                  {t('care.clearPostpone')}
                </Button>
              )}
            </div>
          </article>
        ))}
        {view.page.total === 0 && (
          <div className="flex h-full min-h-32 items-center justify-center">
            <Empty icon={<Leaf />} title={t('care.empty')} description={t('care.emptyHint')} />
          </div>
        )}
      </div>
      <PageBar page={view.page} busy={busy} change={change} />
    </div>
  )
}
export function Timeline({ view, busy, confirm, change }: { view: View; busy: boolean; confirm: (op: Op) => void; change: (offset: number) => void }) {
  const { t } = usePlugin()
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="min-h-0 flex-1 space-y-2 overflow-auto pr-1">
        {(view.page.items as Entry[]).map((row) => (
          <article key={row.id} className="flex items-start gap-3 rounded-card border border-border p-3">
            {row.thumbnail ? (
              <img src={row.thumbnail} alt={t('photos.alt', { date: row.date })} className="size-14 shrink-0 rounded-control object-cover" />
            ) : (
              <div className="flex size-9 shrink-0 items-center justify-center rounded-control bg-primary-soft text-primary">
                <Leaf className="size-4" />
              </div>
            )}
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-2">
                <span className="text-xs font-medium tabular-nums text-fg">{row.date}</span>
                <Badge variant={row.voided ? 'warning' : 'neutral'}>
                  {t(row.kind === 'photo' ? 'timeline.photo' : row.voided ? 'timeline.voided' : 'timeline.completed')}
                </Badge>
              </div>
              <p className="mt-1 text-sm font-medium text-fg">{row.name}</p>
              <p className="mt-1 text-xs leading-relaxed whitespace-pre-wrap break-words text-fg-muted">{row.note}</p>
            </div>
            {row.kind === 'care' && (
              <Button size="sm" variant="ghost" disabled={busy} onClick={() => confirm({ type: 'void', record: row.id, voided: !row.voided })}>
                <RotateCcw />
                {t(row.voided ? 'timeline.restore' : 'timeline.undo')}
              </Button>
            )}
          </article>
        ))}
        {view.page.total === 0 && <Empty icon={<Leaf />} title={t('timeline.empty')} />}
      </div>
      <PageBar page={view.page} busy={busy} change={change} />
    </div>
  )
}
export function Gallery({
  view,
  busy,
  picked,
  pick,
  compare,
  add,
  confirm,
  action,
  change,
}: {
  view: View
  busy: boolean
  picked: string[]
  pick: (id: string) => void
  compare: () => void
  add: () => void
  confirm: (op: Op) => void
  action: (op: Op) => void
  change: (offset: number) => void
}) {
  const { t } = usePlugin()
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-2">
        <p className="text-xs text-fg-muted">{t('photos.selectHint')}</p>
        <div className="flex gap-2">
          <Button size="sm" variant="outline" disabled={busy || picked.length === 0} onClick={compare}>
            {t('photos.compare', { n: picked.length })}
          </Button>
          <Button size="sm" variant="primary" disabled={busy || !view.selected} onClick={add}>
            <Camera />
            {t('photos.add')}
          </Button>
        </div>
      </div>
      <div className="min-h-0 flex-1 overflow-auto">
        <div className="grid grid-cols-2 gap-3 @min-[60rem]:grid-cols-3">
          {(view.page.items as Entry[]).map((photo) => (
            <article key={photo.id} className="min-w-0 overflow-hidden rounded-card border border-border bg-surface-2">
              <img src={photo.thumbnail} alt={t('photos.alt', { date: photo.date })} className="aspect-[4/3] w-full bg-surface object-contain" />
              <div className="space-y-2 p-3">
                <div className="flex items-center justify-between gap-2">
                  <Checkbox
                    disabled={busy || (picked.length >= 2 && !picked.includes(photo.id))}
                    checked={picked.includes(photo.id)}
                    onCheckedChange={() => pick(photo.id)}
                    aria-label={t('photos.select', { date: photo.date })}
                  >
                    <span className="text-xs tabular-nums">{photo.date}</span>
                  </Checkbox>
                  {view.selected?.cover === photo.id && <Star className="size-3.5 text-primary" />}
                </div>
                <p className="line-clamp-2 text-xs text-fg-muted">{photo.note}</p>
                <div className="flex gap-1">
                  <Tooltip content={t('photos.cover')}>
                    <Button
                      size="icon-sm"
                      variant="ghost"
                      disabled={busy || view.selected?.cover === photo.id}
                      aria-label={t('photos.cover')}
                      onClick={() => action({ type: 'cover', plant: view.selected?.id, photo: photo.id })}
                    >
                      <Star />
                    </Button>
                  </Tooltip>
                  <Tooltip content={t('photos.delete')}>
                    <Button
                      size="icon-sm"
                      variant="ghost"
                      disabled={busy}
                      aria-label={t('photos.delete')}
                      onClick={() => confirm({ type: 'removePhoto', photo: photo.id, confirm: true })}
                    >
                      <Trash2 />
                    </Button>
                  </Tooltip>
                </div>
              </div>
            </article>
          ))}
        </div>
        {view.page.total === 0 && (
          <div className="py-10">
            <Empty icon={<Camera />} title={t('photos.empty')} description={t('photos.policy')} />
          </div>
        )}
      </div>
      <PageBar page={view.page} busy={busy} change={change} />
    </div>
  )
}
