import { Button, Empty, Panel, Tooltip } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { AppWindow, EyeOff } from 'lucide-react'
import type { Summary } from './types'
import { useFormat } from './useFormat'

export function Apps({ summary, onExclude }: { summary: Summary; onExclude: (id: string) => void }) {
  const { t } = usePlugin()
  const format = useFormat()
  const apps = summary.apps
  const max = apps.reduce((m, a) => Math.max(m, a.active), 0)
  const total = summary.totals.active

  if (apps.length === 0) {
    return (
      <Panel className="h-full">
        <Empty icon={<AppWindow />} title={t('empty.apps')} description={t('apps.emptyHint')} className="h-full" />
      </Panel>
    )
  }

  return (
    <Panel title={t('apps.title')} icon={<AppWindow />} extra={<span className="text-xs text-fg-muted">{t('apps.hint')}</span>}>
      <div className="overflow-x-auto">
        <table className="w-full min-w-[560px] text-[13px]">
          <thead>
            <tr className="border-b border-border text-left text-xs text-fg-muted">
              <th className="px-3 py-2 font-medium">{t('apps.name')}</th>
              <th className="w-[38%] px-3 py-2 font-medium">{t('apps.active')}</th>
              <th className="px-3 py-2 text-right font-medium">{t('stats.keys')}</th>
              <th className="px-3 py-2 text-right font-medium">{t('stats.clicks')}</th>
              <th className="w-10 px-2 py-2" />
            </tr>
          </thead>
          <tbody>
            {apps.map((app) => (
              <tr key={app.id} className="group border-b border-border last:border-0 hover:bg-hover">
                <td className="max-w-0 px-3 py-2">
                  <div className="truncate text-fg">{app.name}</div>
                  {app.id !== app.name && <div className="truncate text-xs text-fg-subtle">{app.id}</div>}
                </td>
                <td className="px-3 py-2">
                  <div className="flex items-center gap-2">
                    <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-surface-2">
                      <div className="h-full rounded-full bg-primary" style={{ width: `${max > 0 ? (app.active / max) * 100 : 0}%` }} />
                    </div>
                    <span className="w-24 shrink-0 text-right text-xs tabular-nums text-fg-muted">
                      {format.duration(app.active)}
                      {total > 0 && <span className="ml-1 text-fg-subtle">{format.percent(app.active / total)}</span>}
                    </span>
                  </div>
                </td>
                <td className="px-3 py-2 text-right tabular-nums text-fg-muted">{format.count(app.keys)}</td>
                <td className="px-3 py-2 text-right tabular-nums text-fg-muted">{format.count(app.clicks)}</td>
                <td className="px-2 py-2">
                  <Tooltip content={t('apps.exclude')}>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label={t('apps.exclude')}
                      className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                      onClick={() => onExclude(app.id)}
                    >
                      <EyeOff />
                    </Button>
                  </Tooltip>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </Panel>
  )
}
