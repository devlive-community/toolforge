import { useState, type ReactNode } from 'react'
import { cn } from '@toolforge/ui'

export function StatCard({ icon, label, value, hint }: { icon: ReactNode; label: ReactNode; value: ReactNode; hint?: ReactNode }) {
  return (
    <div className="flex min-w-0 flex-col gap-1.5 rounded-card border border-border bg-surface p-3.5 shadow-card">
      <div className="flex items-center gap-1.5 text-xs text-fg-muted [&_svg]:size-3.5">
        {icon}
        <span className="truncate">{label}</span>
      </div>
      <div className="truncate text-xl font-semibold tabular-nums text-fg">{value}</div>
      {hint && <div className="truncate text-xs text-fg-subtle">{hint}</div>}
    </div>
  )
}

export interface RankItem {
  key: string
  label: ReactNode
  count: number
}

/** 排行列表：名称 + 占比条 + 次数 */
export function RankList({
  items,
  format,
  empty,
  action,
}: {
  items: RankItem[]
  format: (n: number) => string
  empty: ReactNode
  action?: (item: RankItem) => ReactNode
}) {
  const max = items.reduce((m, item) => Math.max(m, item.count), 0)
  if (items.length === 0) return <p className="px-3 py-6 text-center text-xs text-fg-subtle">{empty}</p>
  return (
    <ul className="flex flex-col gap-1 p-2">
      {items.map((item, index) => (
        <li key={item.key} className="group flex items-center gap-2 rounded-control px-1.5 py-1 hover:bg-hover">
          <span className="w-5 shrink-0 text-right text-xs tabular-nums text-fg-subtle">{index + 1}</span>
          <div className="min-w-0 flex-1">
            <div className="flex items-center justify-between gap-2">
              <span className="truncate text-[13px] text-fg">{item.label}</span>
              <span className="shrink-0 text-xs tabular-nums text-fg-muted">{format(item.count)}</span>
            </div>
            <div className="mt-1 h-1 overflow-hidden rounded-full bg-surface-2">
              <div className="h-full rounded-full bg-primary" style={{ width: `${max > 0 ? (item.count / max) * 100 : 0}%` }} />
            </div>
          </div>
          {action?.(item)}
        </li>
      ))}
    </ul>
  )
}

/** 柱状图：悬停的柱子在标题行显示数值 */
export function BarChart({
  values,
  labels,
  ticks,
  format,
  className,
}: {
  values: number[]
  labels: string[]
  /** 需要在横轴显示文字的下标 */
  ticks: number[]
  format: (value: number, index: number) => string
  className?: string
}) {
  const [hover, setHover] = useState<number | null>(null)
  const max = values.reduce((m, v) => Math.max(m, v), 0)
  return (
    <div className={cn('flex flex-col gap-2', className)}>
      <div className="h-4 text-xs tabular-nums text-fg-muted">
        {hover !== null ? `${labels[hover]} · ${format(values[hover], hover)}` : ''}
      </div>
      <div className="flex min-h-0 flex-1 items-end gap-[3px]" onMouseLeave={() => setHover(null)}>
        {values.map((value, index) => (
          <div
            key={labels[index]}
            className="flex h-full min-w-0 flex-1 items-end rounded-[3px] hover:bg-hover"
            onMouseEnter={() => setHover(index)}
          >
            <div
              className={cn('w-full rounded-[3px] transition-[height]', hover === index ? 'bg-primary-hover' : 'bg-primary')}
              style={{ height: `${max > 0 ? Math.max((value / max) * 100, value > 0 ? 2 : 0) : 0}%` }}
            />
          </div>
        ))}
      </div>
      <div className="flex gap-[3px] text-[10px] text-fg-subtle">
        {values.map((_, index) => (
          <span key={labels[index]} className="min-w-0 flex-1 overflow-visible whitespace-nowrap text-center">
            {ticks.includes(index) ? labels[index] : ''}
          </span>
        ))}
      </div>
    </div>
  )
}
