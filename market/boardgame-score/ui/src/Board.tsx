import { Badge, Button, Pagination, cn } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { Trophy } from 'lucide-react'
import type { Game, Page, Round } from './types'
export function PageBar({ page, change, busy }: { page: Page<unknown>; change: (offset: number) => void; busy?: boolean }) {
  const { t } = usePlugin()
  return (
    <div className={busy ? 'pointer-events-none opacity-50' : ''}>
      <Pagination
        page={page.offset / page.limit + 1}
        pageSize={page.limit}
        total={page.total}
        onPageChange={(n) => {
          if (!busy) change((n - 1) * page.limit)
        }}
        prevLabel={t('page.prev')}
        nextLabel={t('page.next')}
        pageLabel={(n) => t('page.number', { n })}
        totalLabel={t('page.total', { n: page.total })}
      />
    </div>
  )
}
export function Standings({ game }: { game: Game }) {
  const { t } = usePlugin()
  return (
    <div className="grid grid-cols-2 gap-2 @min-[44rem]:grid-cols-3">
      {game.ranking.map((p) => (
        <div
          key={p.seat}
          className={cn('relative min-w-0 rounded-card border p-3', p.leading ? 'border-primary/30 bg-primary-soft' : 'border-border bg-surface-2')}
        >
          <div className="flex items-center gap-2">
            <span
              className={cn(
                'flex size-6 shrink-0 items-center justify-center rounded-full text-[11px] font-semibold',
                p.leading ? 'bg-primary text-primary-fg' : 'bg-hover text-fg-muted',
              )}
            >
              {p.rank}
            </span>
            <span className="truncate text-sm font-medium text-fg">{p.name}</span>
            {p.leading && <Trophy className="ml-auto size-3.5 shrink-0 text-primary" />}
          </div>
          <div className="mt-2 flex items-end justify-between gap-2">
            <span className="text-[11px] text-fg-subtle">{t('seat', { n: p.seat })}</span>
            <span className={cn('text-2xl leading-none font-semibold tabular-nums', p.leading ? 'text-primary' : 'text-fg')}>{p.total}</span>
          </div>
        </div>
      ))}
    </div>
  )
}
export function RoundTable({ game, busy, page, edit }: { game: Game; busy: boolean; page: (offset: number) => void; edit?: (round: Round) => void }) {
  const { t } = usePlugin()
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <p className="text-xs text-fg-subtle">{t('table.hint')}</p>
      <div className="min-h-0 flex-1 overflow-auto rounded-control border border-border">
        <table className="w-full text-left text-xs tabular-nums">
          <thead className="sticky top-0 bg-surface-2 text-fg-muted">
            <tr>
              <th className="p-3 whitespace-nowrap">{t('table.round')}</th>
              {game.players.map((p) => (
                <th key={p.seat} className="min-w-28 p-3">
                  <span className="block max-w-36 truncate font-medium">{p.name}</span>
                  <span className="font-normal text-fg-subtle">{t('seat', { n: p.seat })}</span>
                </th>
              ))}
              {edit && <th className="p-2">{t('table.actions')}</th>}
            </tr>
          </thead>
          <tbody>
            {game.rounds.items.map((r) => (
              <tr key={r.number} className="border-t border-border">
                <td className="p-3 text-fg-muted">{r.number}</td>
                {r.scores.map((score, i) => (
                  <td key={game.players[i].seat} className="p-3">
                    <span className="font-medium text-fg">{score}</span>
                    <span className="mt-1 block text-[11px] text-fg-subtle">{t('table.cumulative', { n: r.cumulative[i] })}</span>
                  </td>
                ))}
                {edit && (
                  <td className="p-2">
                    <Button size="sm" variant="ghost" disabled={busy || game.finished} onClick={() => edit(r)}>
                      {t('table.edit')}
                    </Button>
                  </td>
                )}
              </tr>
            ))}
          </tbody>
        </table>
        {game.rounds.total === 0 && <p className="p-8 text-center text-xs text-fg-subtle">{t('table.empty')}</p>}
      </div>
      <PageBar page={game.rounds} busy={busy} change={page} />
    </div>
  )
}
export function Audit({ game, busy, page }: { game: Game; busy: boolean; page: (offset: number) => void }) {
  const { t } = usePlugin()
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <p className="text-xs text-fg-muted">{t('audit.hint')}</p>
      <div className="min-h-0 flex-1 space-y-2 overflow-auto">
        {game.events.items.map((e) => (
          <article key={e.sequence} className="space-y-2 rounded-card border border-border p-3">
            <div className="flex items-center gap-2">
              <Badge variant="neutral">{t('audit.number', { n: e.sequence })}</Badge>
              <h3 className="text-sm font-medium text-fg">{t(`event.${e.kind}`)}</h3>
            </div>
            {e.beforeText !== null && (
              <p className="text-xs leading-relaxed break-words text-fg-muted">
                {e.beforeText}
                <span className="mx-2">{t('common.arrow')}</span>
                {e.afterText}
              </p>
            )}
            {e.changes.map((c, i) => (
              <p key={i} className="text-xs text-fg-muted">
                {t(c.draft ? 'audit.draftCell' : 'audit.cell', { round: c.round, seat: c.seat })}
                <span className="ml-2 font-mono text-fg">
                  {c.before === null ? t('entry.empty') : c.before}
                  <span className="mx-2 text-fg-subtle">{t('common.arrow')}</span>
                  {c.after === null ? t('entry.empty') : c.after}
                </span>
              </p>
            ))}
          </article>
        ))}
        {game.events.total === 0 && <p className="p-6 text-center text-xs text-fg-subtle">{t('audit.empty')}</p>}
      </div>
      <PageBar page={game.events} busy={busy} change={page} />
    </div>
  )
}
