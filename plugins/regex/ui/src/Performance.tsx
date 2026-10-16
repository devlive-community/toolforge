import { Badge, Empty, Spinner } from '@toolforge/ui'
import { usePlugin, type AppError } from '@toolforge/plugin-ui-sdk'
import { Gauge, ShieldAlert, ShieldCheck } from 'lucide-react'
import type { AnalyzeResult, BenchRun } from './types'

const GROWTH_VARIANT = { linear: 'success', polynomial: 'warning', exponential: 'danger' } as const

function RunRow({ label, run, budget }: { label: string; run: BenchRun; budget: number }) {
  const { t } = usePlugin()
  return (
    <tr className="border-t border-border">
      <td className="py-1.5 pr-3 text-fg-muted">{label}</td>
      <td className={`py-1.5 pr-3 text-right font-mono tabular-nums ${run.exceeded ? 'text-danger' : 'text-fg'}`}>
        {run.exceeded ? t('perf.exceeded', { budget: budget.toLocaleString() }) : run.steps.toLocaleString()}
      </td>
      <td className="py-1.5 pr-3 text-right font-mono text-fg-subtle tabular-nums">{t('status.elapsed', { ms: run.elapsedMs })}</td>
      <td className="py-1.5 text-right text-fg-subtle">{run.exceeded ? '—' : t(run.matched ? 'perf.matched' : 'perf.noMatch')}</td>
    </tr>
  )
}

export function Performance({ result, error, pending }: { result: AnalyzeResult | null; error: AppError | null; pending: boolean }) {
  const { t, errorMessage } = usePlugin()
  if (error) return <p className="p-4 text-xs text-danger">{errorMessage(error)}</p>
  if (!result) {
    return pending ? (
      <div className="flex h-full items-center justify-center text-fg-muted">
        <Spinner />
      </div>
    ) : (
      <Empty icon={<Gauge />} title={t('perf.empty')} />
    )
  }
  return (
    <div className="space-y-4 p-3 text-xs">
      <p className="rounded-control bg-surface-2 px-3 py-2 leading-relaxed text-fg-muted">
        {t(result.backtrackingEngine ? 'perf.engineBacktracking' : 'perf.engineLinear')}
      </p>

      <section className="space-y-2">
        <h3 className="flex items-center gap-1.5 text-[13px] font-semibold text-fg">
          {result.findings.length > 0 ? <ShieldAlert className="size-4 text-danger" /> : <ShieldCheck className="size-4 text-success" />}
          {t('perf.static')}
        </h3>
        {result.findings.length === 0 ? (
          <p className="text-fg-muted">{t('perf.noFindings')}</p>
        ) : (
          result.findings.map((finding, index) => (
            <div key={index} className="space-y-1.5 rounded-control border border-border p-2.5">
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant={finding.severity === 'high' ? 'danger' : 'warning'}>{t(`perf.severity.${finding.severity}`)}</Badge>
                <span className="font-medium text-fg">{t(`perf.kinds.${finding.kind}.title`)}</span>
                <code className="rounded-sm bg-surface-2 px-1.5 py-0.5 font-mono text-fg" data-selectable>
                  {finding.snippet}
                </code>
              </div>
              <p className="leading-relaxed text-fg-muted">{t(`perf.kinds.${finding.kind}.explain`)}</p>
              <p className="leading-relaxed text-fg">{t(`perf.kinds.${finding.kind}.fix`)}</p>
              <p className="text-fg-subtle">
                {t('perf.attack')}{' '}
                <code className="font-mono break-all text-fg-muted" data-selectable>
                  {JSON.stringify(finding.attack)}
                </code>
              </p>
            </div>
          ))
        )}
      </section>

      <section className="space-y-2">
        <h3 className="flex items-center gap-1.5 text-[13px] font-semibold text-fg">
          <Gauge className="size-4 text-fg-muted" />
          {t('perf.dynamic')}
          {result.growth && <Badge variant={GROWTH_VARIANT[result.growth]}>{t(`perf.growth.${result.growth}`)}</Badge>}
        </h3>
        {!result.simulated ? (
          <p className="text-fg-muted">{t('perf.unsupported')}</p>
        ) : (
          <>
            <p className="text-fg-subtle">{t('perf.dynamicHint')}</p>
            <table className="w-full">
              <thead>
                <tr className="text-left text-fg-subtle">
                  <th className="pb-1 font-normal">{t('perf.input')}</th>
                  <th className="pb-1 text-right font-normal">{t('perf.steps')}</th>
                  <th className="pb-1 text-right font-normal">{t('perf.time')}</th>
                  <th className="pb-1 text-right font-normal">{t('perf.result')}</th>
                </tr>
              </thead>
              <tbody>
                {result.sample && (
                  <RunRow label={t(result.sampleTruncated ? 'perf.sampleTruncated' : 'perf.sample', { length: result.sample.length })} run={result.sample} budget={result.stepBudget} />
                )}
                {result.attacks.map((run) => (
                  <RunRow key={run.length} label={t('perf.attackRun', { length: run.length })} run={run} budget={result.stepBudget} />
                ))}
              </tbody>
            </table>
          </>
        )}
      </section>
    </div>
  )
}
