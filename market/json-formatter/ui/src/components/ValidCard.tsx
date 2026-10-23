import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { CircleCheck } from 'lucide-react'
import type { Stats } from '../types'

export function ValidCard({ stats, elapsedMs }: { stats: Stats; elapsedMs: number }) {
  const { t } = usePlugin()
  const format = new Intl.NumberFormat()
  const items: [string, number][] = [
    [t('stats.objects'), stats.objects],
    [t('stats.arrays'), stats.arrays],
    [t('stats.keys'), stats.keys],
    [t('stats.maxDepth'), stats.maxDepth],
  ]
  return (
    <div className="flex h-full items-start justify-center overflow-auto p-6">
      <div className="w-full max-w-md animate-fade-in rounded-card border border-success/30 bg-success-soft/60 p-4">
        <div className="flex items-center gap-2 text-success">
          <CircleCheck className="size-4" />
          <p className="text-[13px] font-semibold">{t('validate.ok')}</p>
        </div>
        <p className="mt-1.5 text-xs text-fg-muted">{t('validate.elapsed', { ms: elapsedMs })}</p>
        <dl className="mt-3 grid grid-cols-4 gap-2">
          {items.map(([label, value]) => (
            <div key={label} className="rounded-control bg-surface/80 px-2.5 py-2">
              <dt className="text-[11px] text-fg-muted">{label}</dt>
              <dd className="mt-0.5 font-mono text-sm text-fg">{format.format(value)}</dd>
            </div>
          ))}
        </dl>
      </div>
    </div>
  )
}
