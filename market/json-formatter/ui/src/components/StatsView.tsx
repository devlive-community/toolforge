import { Empty } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { ChartColumn } from 'lucide-react'
import type { Stats } from '../types'

const ROWS: (keyof Stats)[] = ['objects', 'arrays', 'keys', 'strings', 'numbers', 'booleans', 'nulls', 'maxDepth', 'lines', 'chars', 'bytes']

export function StatsView({ stats }: { stats: Stats | null }) {
  const { t } = usePlugin()
  if (!stats) return <Empty icon={<ChartColumn />} title={t('stats.empty')} />
  const format = new Intl.NumberFormat()
  return (
    <dl className="divide-y divide-border px-3 py-1">
      {ROWS.map((key) => (
        <div key={key} className="flex items-center justify-between py-2 text-[13px]">
          <dt className="text-fg-muted">{t(`stats.${key}`)}</dt>
          <dd className="font-mono text-fg tabular-nums" data-selectable>
            {format.format(stats[key])}
          </dd>
        </div>
      ))}
    </dl>
  )
}
