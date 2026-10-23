import { useEffect, useRef, useState } from 'react'
import { usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { Badge, Empty, Panel, Select, Spinner } from '@toolforge/ui'
import { BarChart3, Lightbulb, Receipt, TrendingDown } from 'lucide-react'
import { Pages } from './Editors'
import type { Page, Report, Row } from './types'
function Distribution({ handle, tariff }: { handle: string; tariff: number }) {
  const { t, errorMessage } = usePlugin()
  const task = useTask<Page<Row>>()
  const [offset, setOffset] = useState(0)
  const start = useRef(task.start)
  useEffect(() => {
    start.current = task.start
  })
  useEffect(() => {
    void start.current('query', { handle, tariff, offset, limit: 8 })
  }, [handle, tariff, offset])
  return (
    <div className="space-y-3">
      {task.error && (
        <p role="alert" className="text-xs text-danger">
          {errorMessage(task.error)}
        </p>
      )}
      {task.running ? (
        <Spinner className="size-4" />
      ) : (
        task.result?.items.map((row) => (
          <article key={row.id} className="space-y-2 rounded-control border border-border p-3">
            <div className="flex justify-between gap-3 text-xs">
              <span className="truncate font-medium text-fg">{row.name}</span>
              <span className="font-mono tabular-nums text-fg-muted">{t('result.share', { value: row.sharePercent })}</span>
            </div>
            <div className="h-1.5 overflow-hidden rounded-full bg-surface-3">
              <div className="h-full rounded-full bg-primary" style={{ width: `${row.sharePercent}%` }} />
            </div>
            <div className="grid grid-cols-2 gap-3 text-[11px] text-fg-muted">
              <span>{t('result.rowBase', { energy: row.baselineKwh, cost: row.baselineCost })}</span>
              <span>{t('result.rowAlt', { energy: row.alternativeKwh, cost: row.alternativeCost })}</span>
            </div>
          </article>
        ))
      )}
      {task.result && <Pages data={task.result} change={setOffset} />}
    </div>
  )
}
export function Results({ report }: { report: Report | null }) {
  const { t } = usePlugin()
  const [selected, setSelected] = useState('0')
  if (!report)
    return (
      <Panel className="min-h-0 flex-1" bodyClassName="flex h-full items-center justify-center p-3">
        <Empty icon={<Lightbulb />} title={t('result.empty')} description={t('result.emptyHint')} />
      </Panel>
    )
  const index = Number(selected)
  const b = report.summaries[index] ?? report.summaries[0]
  return (
    <Panel className="min-h-0 flex-1" title={t('result.title')} icon={<Receipt />} bodyClassName="h-full min-h-0 space-y-4 overflow-auto p-3">
      <Select
        size="sm"
        value={selected}
        onValueChange={setSelected}
        aria-label={t('result.plan')}
        options={report.summaries.map((s, i) => ({ value: String(i), label: s.tariff.name }))}
      />
      <div className="rounded-card border border-primary/20 bg-primary/5 p-4">
        <p className="text-xs text-fg-muted">{t('result.monthlyCost')}</p>
        <div className="mt-2 flex items-baseline gap-2">
          <span className="text-3xl font-semibold tracking-tight tabular-nums text-fg">{b.baseline.monthlyCost}</span>
          <span className="text-xs text-fg-muted">{report.currency}</span>
        </div>
        <div className="mt-4 grid grid-cols-3 gap-3 border-t border-primary/15 pt-3 text-[11px] text-fg-muted">
          <div>
            <p>{t('result.monthlyEnergy')}</p>
            <p className="mt-1 font-mono text-fg">{t('result.kwh', { n: b.baseline.monthlyKwh })}</p>
          </div>
          <div>
            <p>{t('result.dailyAverage')}</p>
            <p className="mt-1 font-mono text-fg">{b.baseline.dailyCost}</p>
          </div>
          <div>
            <p>{t('result.annual')}</p>
            <p className="mt-1 font-mono text-fg">{b.baseline.annualCost}</p>
          </div>
        </div>
      </div>
      <p className="text-[11px] leading-relaxed text-fg-subtle">{t('result.period', { days: report.days, count: report.applianceCount })}</p>
      <section className="space-y-3 rounded-card border border-border p-3">
        <div className="flex items-center gap-2 text-xs font-medium text-fg">
          <TrendingDown className="size-4 text-primary" />
          {t('result.alternative')}
        </div>
        <div className="grid grid-cols-2 gap-3">
          <div>
            <p className="text-[11px] text-fg-muted">{t('result.monthlyCost')}</p>
            <p className="mt-1 text-lg font-semibold tabular-nums text-fg">{b.alternative.monthlyCost}</p>
          </div>
          <div>
            <p className="text-[11px] text-fg-muted">{t(`saving.${b.savingDirection}`)}</p>
            <p className="mt-1 text-lg font-semibold tabular-nums text-fg">{b.annualSaving}</p>
          </div>
        </div>
        <p className="text-[11px] text-fg-subtle">{t('result.noAutomaticSavings')}</p>
      </section>
      {b.baseline.bands.length > 0 && (
        <section className="space-y-2">
          <h3 className="text-xs font-medium text-fg">{t(b.tariff.pricing.kind === 'tiered' ? 'result.tiers' : 'result.periods')}</h3>
          {b.baseline.bands.map((band) => (
            <div key={band.index} className="flex flex-wrap items-center gap-2 rounded-control bg-surface-2 p-2 text-[11px] text-fg-muted">
              <Badge variant={band.reached ? 'primary' : 'neutral'}>
                {b.tariff.pricing.kind === 'tiered'
                  ? t('result.tier', { n: band.index + 1 })
                  : t(`period.${(['peak', 'shoulder', 'valley'] as const)[band.index]}`)}
              </Badge>
              <span>{t('result.band', { kwh: band.kwh, rate: band.rate, cost: band.cost })}</span>
            </div>
          ))}
          {b.tariff.pricing.kind === 'tiered' && (
            <p className="text-[11px] text-warning">
              {b.baseline.highestTier === null ? t('result.noUsage') : t('result.highest', { n: b.baseline.highestTier + 1 })}
              {b.baseline.remainingKwh !== null && t('result.remaining', { n: b.baseline.remainingKwh })}
            </p>
          )}
        </section>
      )}
      <section className="space-y-3">
        <h3 className="flex items-center gap-2 text-xs font-medium text-fg">
          <BarChart3 className="size-4" />
          {t('result.distribution')}
        </h3>
        <p className="text-[11px] text-fg-subtle">{t('result.allocation')}</p>
        <Distribution key={`${report.handle}:${index}`} handle={report.handle} tariff={index} />
      </section>
    </Panel>
  )
}
