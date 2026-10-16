import { useState } from 'react'
import { Badge, Panel, SegmentedControl, cn } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Sparkles, Stars } from 'lucide-react'
import { DateInput } from './DateInput'
import { LunarPicker } from './LunarPicker'
import type { Birth, LunarDate } from './types'

type Kind = 'solar' | 'lunar'

/** 生肖与星座快速查询（#307）：生日可按公历或农历输入 */
export function BirthTab({ today }: { today: string }) {
  const { t, errorMessage } = usePlugin()
  const [kind, setKind] = useState<Kind>('solar')
  const [date, setDate] = useState('2000-01-01')
  const [lunar, setLunar] = useState<LunarDate>({ year: 2000, month: 1, day: 1, leap: false })
  const args = kind === 'solar' ? { calendar: 'solar', date } : { calendar: 'lunar', ...lunar }
  const { result, error } = useDebouncedCall<Birth>('birth', args, [kind, date, lunar.year, lunar.month, lunar.day, lunar.leap])
  const differs = result && result.zodiacLichun !== result.zodiacLunarYear

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(320px,0.9fr)_minmax(0,1.1fr)] gap-3">
      <Panel icon={<Sparkles />} title={t('birth.title')} bodyClassName="space-y-4 overflow-auto p-4">
        <SegmentedControl<Kind>
          value={kind}
          onValueChange={setKind}
          aria-label={t('birth.kind')}
          options={[
            { value: 'solar', label: t('birth.solar') },
            { value: 'lunar', label: t('birth.lunar') },
          ]}
        />
        {kind === 'solar' ? (
          <DateInput label={t('birth.birthday')} value={date} onChange={setDate} today={today} />
        ) : (
          <div className="space-y-1.5">
            <p className="text-xs font-medium text-fg-muted">{t('birth.birthday')}</p>
            <LunarPicker value={lunar} onChange={setLunar} />
          </div>
        )}
        {error ? (
          <p className="text-[13px] text-danger">{errorMessage(error)}</p>
        ) : (
          result && (
            <div className="space-y-3">
              <p className="text-xs text-fg-muted" data-selectable>
                {t('birth.converted', { date: result.date, lunar: result.lunar })}
              </p>
              <div className="grid grid-cols-2 gap-2">
                <div className="rounded-card border border-border p-3">
                  <p className="text-xs text-fg-muted">{t('birth.zodiac')}</p>
                  <p className="mt-1 text-2xl font-semibold text-fg">{t(`zodiac.${result.zodiacLunarYear}`)}</p>
                  <p className="text-[11px] text-fg-subtle">{t('birth.byLunarYear', { ganzhi: result.lunarYearGanzhi })}</p>
                </div>
                <div className="rounded-card border border-border p-3">
                  <p className="text-xs text-fg-muted">{t('birth.constellation')}</p>
                  <p className="mt-1 text-2xl font-semibold text-fg">{t(`constellations.${result.constellation}`)}</p>
                  <p className="text-[11px] text-fg-subtle">{t('birth.bySolar')}</p>
                </div>
              </div>
              <div className={cn('rounded-control p-2.5 text-xs leading-relaxed', differs ? 'bg-warning-soft text-fg' : 'bg-surface-2 text-fg-muted')}>
                {differs
                  ? t('birth.differs', {
                      lunar: t(`zodiac.${result.zodiacLunarYear}`),
                      lichun: t(`zodiac.${result.zodiacLichun}`),
                      lichunDate: result.lichun,
                      spring: result.springFestival,
                      ganzhi: result.lichunYearGanzhi,
                    })
                  : t('birth.same', { lichunDate: result.lichun, spring: result.springFestival })}
              </div>
            </div>
          )
        )}
      </Panel>

      <Panel icon={<Stars />} title={t('birth.table')} bodyClassName="overflow-auto p-3">
        {result && (
          <ul className="space-y-1.5">
            {result.constellations.map((range) => (
              <li
                key={range.index}
                className={cn('rounded-control border px-3 py-2', range.index === result.constellation ? 'border-primary bg-primary-soft' : 'border-border')}
              >
                <div className="flex items-center gap-2">
                  <span className="text-[13px] font-medium text-fg">{t(`constellations.${range.index}`)}</span>
                  <span className="text-xs text-fg-muted tabular-nums">
                    {t('birth.range', { sm: range.start[0], sd: range.start[1], em: range.end[0], ed: range.end[1] })}
                  </span>
                  {range.index === result.constellation && <Badge variant="primary">{t('birth.yours')}</Badge>}
                </div>
                <p className="mt-0.5 text-xs text-fg-muted">{t(`traits.${range.index}`)}</p>
              </li>
            ))}
          </ul>
        )}
        <p className="mt-3 text-[11px] text-fg-subtle">{t('birth.entertainment')}</p>
      </Panel>
    </div>
  )
}
