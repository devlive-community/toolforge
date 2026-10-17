import type { ReactNode } from 'react'
import { Badge, Panel, cn } from '@toolforge/ui'
import { formatBytes, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Boxes, CircleCheck, CircleX, Cpu, FileCode, Flame, Puzzle, Tag, TriangleAlert } from 'lucide-react'
import { functionLabel, hex, percent } from './format'
import type { FunctionRow, Limits, Overview } from './types'

function Stat({ label, value, hint }: { label: ReactNode; value: ReactNode; hint?: ReactNode }) {
  return (
    <div className="flex min-w-0 flex-col gap-1 rounded-card border border-border bg-surface p-3 shadow-card">
      <span className="truncate text-xs text-fg-muted">{label}</span>
      <span className="truncate text-lg font-semibold tabular-nums text-fg">{value}</span>
      {hint && <span className="truncate text-xs text-fg-subtle">{hint}</span>}
    </div>
  )
}

function ValidationPanel({ overview }: { overview: Overview }) {
  const { t, errorMessage } = usePlugin()
  const v = overview.validation
  const icon =
    v.status === 'valid' ? (
      <CircleCheck className="size-4 text-success" />
    ) : v.status === 'needs_features' ? (
      <TriangleAlert className="size-4 text-warning" />
    ) : (
      <CircleX className="size-4 text-danger" />
    )
  const problem = overview.parseError ?? v.error
  return (
    <Panel title={t('validation.title')} icon={<CircleCheck />} bodyClassName="flex flex-col gap-2 p-3">
      <div className="flex items-center gap-2 text-[13px] font-medium text-fg">
        {icon}
        {t(`validation.${v.status}`)}
      </div>
      {problem && (
        <div className="rounded-control border border-border bg-surface-2 px-3 py-2 text-xs">
          <div className="text-fg">{errorMessage({ code: problem.code, params: { detail: problem.message } })}</div>
          {problem.offset !== null && (
            <div className="mt-1 font-mono text-fg-muted">{t('validation.offset', { hex: hex(problem.offset), dec: problem.offset })}</div>
          )}
        </div>
      )}
      {v.status === 'needs_features' && (
        <div className="flex flex-wrap items-center gap-1.5 text-xs text-fg-muted">
          <span>{v.detected ? t('validation.required') : t('validation.requiredUnknown')}</span>
          {v.required.map((f) => (
            <Badge key={f} variant="warning">
              {f}
            </Badge>
          ))}
        </div>
      )}
      <details className="text-xs text-fg-muted">
        <summary className="cursor-pointer select-none">{t('validation.features', { count: v.features.length })}</summary>
        <div className="mt-2 flex flex-wrap gap-1">
          {v.features.map((f) => (
            <span key={f} className="rounded-[5px] border border-border bg-surface-2 px-1.5 py-0.5 font-mono">
              {f}
            </span>
          ))}
        </div>
        <p className="mt-2">{t('validation.featuresHint')}</p>
      </details>
    </Panel>
  )
}

function LimitRow({ l, kind }: { l: Limits; kind: 'memory' | 'table' }) {
  const { t } = usePlugin()
  const unit = kind === 'memory' && l.pageSize ? l.pageSize : null
  const bytes = (pages: number) => (unit ? ` · ${formatBytes(pages * unit)}` : '')
  return (
    <li className="flex flex-wrap items-baseline gap-x-2 gap-y-0.5 text-[13px]">
      <span className="font-mono text-xs text-fg-muted">
        {kind}[{l.index}]
      </span>
      <span className="tabular-nums text-fg">
        {t('limits.min', { value: l.min })}
        {bytes(l.min)}
      </span>
      <span className="tabular-nums text-fg">
        {l.max === null ? t('limits.noMax') : `${t('limits.max', { value: l.max })}${bytes(l.max)}`}
      </span>
      {l.element && <Badge>{l.element}</Badge>}
      {l.is64 && <Badge variant="info">{t('limits.is64')}</Badge>}
      {l.shared && <Badge variant="info">{t('limits.shared')}</Badge>}
      {unit && unit !== 65536 && <Badge variant="warning">{t('limits.pageSize', { size: unit })}</Badge>}
      {l.import && <span className="font-mono text-xs text-fg-subtle">{t('limits.imported', { from: l.import })}</span>}
    </li>
  )
}

export function OverviewView({ overview, onOpenFunction }: { overview: Overview; onOpenFunction: (f: FunctionRow) => void }) {
  const { t } = usePlugin()
  const { sizes, counts } = overview
  const maxSection = overview.sections.reduce((m, s) => Math.max(m, s.size), 0)
  const customs = overview.sections.filter((s) => s.kind === 'custom')

  return (
    <div className="flex flex-col gap-4">
      <div className="grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-6">
        <Stat label={t('stats.file')} value={formatBytes(sizes.total)} hint={t('stats.version', { version: overview.version ?? '—' })} />
        <Stat label={t('stats.withoutCustom')} value={formatBytes(sizes.withoutCustom)} hint={t('stats.custom', { size: formatBytes(sizes.custom) })} />
        <Stat label={t('stats.code')} value={formatBytes(sizes.code)} hint={percent(sizes.code, sizes.total)} />
        <Stat
          label={t('stats.functions')}
          value={counts.functions}
          hint={t('stats.importedFunctions', { count: counts.importedFunctions })}
        />
        <Stat label={t('stats.imports')} value={overview.imports} hint={t('stats.types', { count: counts.types })} />
        <Stat
          label={t('stats.exports')}
          value={overview.exports}
          hint={counts.start !== null ? t('stats.start', { index: counts.start }) : t('stats.noStart')}
        />
      </div>

      <div className="grid gap-4 lg:grid-cols-[3fr_2fr]">
        <Panel title={t('sections.title')} icon={<Boxes />}>
          <table className="w-full text-[13px]">
            <thead>
              <tr className="border-b border-border text-left text-xs text-fg-muted">
                <th className="px-3 py-2 font-medium">{t('sections.kind')}</th>
                <th className="px-3 py-2 text-right font-medium">{t('sections.offset')}</th>
                <th className="px-3 py-2 text-right font-medium">{t('sections.items')}</th>
                <th className="w-[40%] px-3 py-2 font-medium">{t('sections.size')}</th>
              </tr>
            </thead>
            <tbody>
              {overview.sections.map((s) => (
                <tr key={s.offset} className="border-b border-border last:border-0">
                  <td className="px-3 py-1.5">
                    <span className={cn('font-mono text-xs', s.kind === 'custom' ? 'text-fg-muted' : 'text-fg')}>{s.kind}</span>
                    {s.name !== null && <span className="ml-1.5 font-mono text-xs text-fg-subtle">"{s.name}"</span>}
                  </td>
                  <td className="px-3 py-1.5 text-right font-mono text-xs text-fg-subtle">{hex(s.offset)}</td>
                  <td className="px-3 py-1.5 text-right tabular-nums text-fg-muted">{s.items ?? ''}</td>
                  <td className="px-3 py-1.5">
                    <div className="flex items-center gap-2">
                      <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-surface-2">
                        <div
                          className={cn('h-full rounded-full', s.kind === 'custom' ? 'bg-fg-subtle' : 'bg-primary')}
                          style={{ width: `${maxSection > 0 ? (s.size / maxSection) * 100 : 0}%` }}
                        />
                      </div>
                      <span className="w-28 shrink-0 text-right text-xs tabular-nums text-fg-muted">
                        {formatBytes(s.size)} <span className="text-fg-subtle">{percent(s.size, sizes.total)}</span>
                      </span>
                    </div>
                  </td>
                </tr>
              ))}
              {overview.sections.length === 0 && (
                <tr>
                  <td colSpan={4} className="px-3 py-4 text-center text-xs text-fg-subtle">
                    {t('sections.empty')}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </Panel>
        <div className="flex flex-col gap-4">
          <ValidationPanel overview={overview} />
          <Panel title={t('limits.title')} icon={<Cpu />} bodyClassName="p-3">
            {overview.memories.length + overview.tables.length === 0 ? (
              <p className="text-xs text-fg-subtle">{t('limits.empty')}</p>
            ) : (
              <ul className="flex flex-col gap-1.5">
                {overview.memories.map((l) => (
                  <LimitRow key={`m${l.index}`} l={l} kind="memory" />
                ))}
                {overview.tables.map((l) => (
                  <LimitRow key={`t${l.index}`} l={l} kind="table" />
                ))}
              </ul>
            )}
          </Panel>
        </div>
      </div>

      <div className="grid gap-4 lg:grid-cols-[3fr_2fr]">
        <Panel title={t('largest.title')} icon={<Flame />} extra={<span className="text-xs text-fg-muted">{t('largest.hint')}</span>}>
          {overview.largest.length === 0 ? (
            <p className="p-3 text-xs text-fg-subtle">{t('largest.empty')}</p>
          ) : (
            <ul className="flex flex-col p-1.5">
              {overview.largest.map((f) => (
                <li key={f.index}>
                  <button
                    type="button"
                    onClick={() => onOpenFunction(f)}
                    className="flex w-full items-center gap-2 rounded-control px-2 py-1.5 text-left hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
                  >
                    <span className="w-14 shrink-0 font-mono text-xs text-fg-subtle">#{f.index}</span>
                    <span className="min-w-0 flex-1 truncate font-mono text-xs text-fg">{functionLabel(f)}</span>
                    <span className="shrink-0 text-xs tabular-nums text-fg-muted">
                      {formatBytes(f.size)} <span className="text-fg-subtle">{percent(f.size, sizes.code)}</span>
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </Panel>
        <Panel title={t('customs.title')} icon={<Tag />} bodyClassName="flex flex-col gap-3 p-3">
          {customs.length === 0 ? (
            <p className="text-xs text-fg-subtle">{t('customs.empty')}</p>
          ) : (
            <ul className="flex flex-col gap-1">
              {customs.map((s) => (
                <li key={s.offset} className="flex items-center gap-2 text-xs">
                  <FileCode className="size-3.5 shrink-0 text-fg-subtle" />
                  <span className="min-w-0 flex-1 truncate font-mono text-fg">{s.name}</span>
                  <span className="shrink-0 tabular-nums text-fg-muted">{formatBytes(s.size)}</span>
                </li>
              ))}
            </ul>
          )}
          {overview.producers.length > 0 && (
            <div className="border-t border-border pt-3">
              <div className="mb-1.5 flex items-center gap-1.5 text-xs font-medium text-fg">
                <Puzzle className="size-3.5" />
                {t('customs.producers')}
              </div>
              <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-xs">
                {overview.producers.map((p) => (
                  <div key={p.field} className="contents">
                    <dt className="font-mono text-fg-muted">{p.field}</dt>
                    <dd className="font-mono break-all text-fg" data-selectable>
                      {p.values.join(', ')}
                    </dd>
                  </div>
                ))}
              </dl>
            </div>
          )}
        </Panel>
      </div>
    </div>
  )
}
