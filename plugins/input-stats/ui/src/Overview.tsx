import { Panel } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { AppWindow, CalendarDays, Clock, Keyboard, Mouse, MousePointerClick, Move, ScrollText } from 'lucide-react'
import { BarChart, RankList, StatCard } from './components'
import type { Summary } from './types'
import { useFormat } from './useFormat'

const HOUR_TICKS = [0, 6, 12, 18, 23]

export function Overview({ summary }: { summary: Summary }) {
  const { t } = usePlugin()
  const format = useFormat()
  const { totals } = summary
  const peak = summary.hours.reduce((best, h, i) => (h.keys + h.clicks > summary.hours[best].keys + summary.hours[best].clicks ? i : best), 0)
  const hasPeak = summary.hours[peak].keys + summary.hours[peak].clicks > 0
  const days = summary.days
  const dayTicks = days.length <= 8 ? days.map((_, i) => i) : [0, Math.floor(days.length / 2), days.length - 1]

  return (
    <div className="flex flex-col gap-4">
      <div className="grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-5">
        <StatCard icon={<Keyboard />} label={t('stats.keys')} value={format.count(totals.keys)} />
        <StatCard icon={<MousePointerClick />} label={t('stats.clicks')} value={format.count(totals.clicks)} />
        <StatCard
          icon={<Move />}
          label={t('stats.distance')}
          value={format.distance(totals.distance)}
          hint={t('stats.distanceHint')}
        />
        <StatCard icon={<ScrollText />} label={t('stats.scroll')} value={t('unit.lines', { value: format.count(totals.scroll) })} />
        <StatCard
          icon={<Clock />}
          label={t('stats.active')}
          value={format.duration(totals.active)}
          hint={hasPeak ? t('stats.peak', { hour: peak }) : undefined}
        />
      </div>
      <div className="grid gap-4 lg:grid-cols-[3fr_2fr]">
        <Panel title={t('overview.hourly')} icon={<Clock />} bodyClassName="p-4">
          <BarChart
            className="h-48"
            values={summary.hours.map((h) => h.keys + h.clicks)}
            labels={summary.hours.map((_, i) => t('overview.hour', { hour: i }))}
            ticks={HOUR_TICKS}
            format={(_, i) =>
              t('overview.hourValue', {
                keys: format.count(summary.hours[i].keys),
                clicks: format.count(summary.hours[i].clicks),
              })
            }
          />
        </Panel>
        <Panel title={t('overview.apps')} icon={<AppWindow />}>
          <RankList
            items={summary.apps.slice(0, 6).map((a) => ({ key: a.id, label: a.name, count: a.active }))}
            format={format.duration}
            empty={t('empty.apps')}
          />
        </Panel>
      </div>
      {days.length > 1 && (
        <Panel title={t('overview.daily')} icon={<CalendarDays />} bodyClassName="p-4">
          <BarChart
            className="h-44"
            values={days.map((d) => d.keys + d.clicks)}
            labels={days.map((d) => d.day.slice(5))}
            ticks={dayTicks}
            format={(_, i) =>
              t('overview.dayValue', {
                keys: format.count(days[i].keys),
                clicks: format.count(days[i].clicks),
                active: format.duration(days[i].active),
              })
            }
          />
        </Panel>
      )}
      <p className="flex items-center gap-1.5 text-xs text-fg-subtle">
        <Mouse className="size-3.5" />
        {t('overview.privacy')}
      </p>
    </div>
  )
}
