import { useState } from 'react'
import { Badge, Button, Panel, SegmentedControl, cn } from '@toolforge/ui'
import { useDebouncedCall, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { CalendarDays, ChevronLeft, ChevronRight } from 'lucide-react'
import type { Cell, Info, Today } from './types'

type WeekStart = 'monday' | 'sunday'
const WEEKDAY_ORDER: Record<WeekStart, number[]> = { monday: [0, 1, 2, 3, 4, 5, 6], sunday: [6, 0, 1, 2, 3, 4, 5] }

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-baseline justify-between gap-3 border-b border-border py-1.5 last:border-0">
      <span className="text-xs text-fg-muted">{label}</span>
      <span className="text-right text-[13px] text-fg" data-selectable>
        {children}
      </span>
    </div>
  )
}

export function CalendarTab({ today }: { today: Today }) {
  const { t, errorMessage } = usePlugin()
  const [weekStart, setWeekStart] = usePluginState<WeekStart>('weekStart', 'monday')
  const [view, setView] = useState({ year: today.year, month: today.month })
  const [selected, setSelected] = useState(today.date)
  const monthArgs = { ...view, mondayFirst: weekStart === 'monday' }
  const cells = useDebouncedCall<Cell[]>('month', monthArgs, [view.year, view.month, weekStart])
  const info = useDebouncedCall<Info>('info', { date: selected }, [selected])
  const shift = (delta: number) =>
    setView((prev) => {
      const index = prev.year * 12 + prev.month - 1 + delta
      return { year: Math.floor(index / 12), month: (index % 12) + 1 }
    })
  const goToday = () => {
    setView({ year: today.year, month: today.month })
    setSelected(today.date)
  }
  const d = info.result

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_300px] gap-3">
      <Panel
        icon={<CalendarDays />}
        title={t('calendar.month', { year: view.year, month: view.month })}
        actions={
          <>
            <SegmentedControl<WeekStart>
              size="sm"
              value={weekStart}
              onValueChange={setWeekStart}
              aria-label={t('calendar.weekStart')}
              options={(['monday', 'sunday'] as WeekStart[]).map((value) => ({ value, label: t(`calendar.starts.${value}`) }))}
            />
            <Button size="icon-sm" variant="ghost" aria-label={t('calendar.previous')} onClick={() => shift(-1)}>
              <ChevronLeft />
            </Button>
            <Button size="sm" onClick={goToday}>
              {t('input.today')}
            </Button>
            <Button size="icon-sm" variant="ghost" aria-label={t('calendar.next')} onClick={() => shift(1)}>
              <ChevronRight />
            </Button>
          </>
        }
        bodyClassName="flex flex-col p-3"
      >
        <div className="grid shrink-0 grid-cols-7 pb-1.5 text-center text-[11px] text-fg-subtle">
          {WEEKDAY_ORDER[weekStart].map((w) => (
            <span key={w}>{t(`weekdaysShort.${w}`)}</span>
          ))}
        </div>
        <div className={cn('grid min-h-0 flex-1 grid-cols-7 grid-rows-6 gap-1', cells.pending && 'opacity-70')}>
          {cells.result?.map((cell) => (
            <button
              key={cell.date}
              type="button"
              onClick={() => setSelected(cell.date)}
              className={cn(
                'relative flex flex-col items-center justify-center rounded-control border outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring',
                cell.date === selected ? 'border-primary bg-primary-soft' : 'border-transparent hover:bg-hover',
                !cell.inMonth && 'opacity-40',
              )}
            >
              {cell.holiday && (
                <span className={cn('absolute top-1 right-1.5 text-[10px] font-medium', cell.holiday.work ? 'text-fg-muted' : 'text-success')}>
                  {t(cell.holiday.work ? 'calendar.work' : 'calendar.rest')}
                </span>
              )}
              <span
                className={cn(
                  'grid size-8 place-items-center rounded-full text-[15px] tabular-nums',
                  // 调休上班的周末不算休息日
                  cell.today ? 'bg-primary text-fg-on-primary' : (cell.holiday ? !cell.holiday.work : cell.weekend) ? 'text-danger' : 'text-fg',
                )}
              >
                {cell.day}
              </span>
              <span className={cn('max-w-full truncate px-1 text-[11px]', cell.special ? 'text-primary' : 'text-fg-subtle')}>{cell.label}</span>
            </button>
          ))}
        </div>
        {view.year > today.holidayDataUntil && <p className="shrink-0 pt-2 text-[11px] text-warning">{t('calendar.noHolidayData', { year: today.holidayDataUntil })}</p>}
      </Panel>

      <Panel title={selected} bodyClassName="overflow-auto p-3">
        {info.error ? (
          <p className="text-[13px] text-danger">{errorMessage(info.error)}</p>
        ) : d ? (
          <div className="space-y-3">
            <div>
              <p className="text-lg font-semibold text-fg">{t(`weekdays.${d.weekday}`)}</p>
              {d.lunar && <p className="text-[13px] text-fg-muted">{d.lunar.text}</p>}
              <div className="mt-1.5 flex flex-wrap gap-1">
                {d.holiday && (
                  <Badge variant={d.holiday.work ? 'neutral' : 'success'}>
                    {t(`holidays.${d.holiday.index}`)} · {t(d.holiday.work ? 'calendar.workday' : 'calendar.restday')}
                  </Badge>
                )}
                {d.festivals.map((f) => (
                  <Badge key={f} variant="primary">
                    {f}
                  </Badge>
                ))}
              </div>
            </div>
            <div>
              <Row label={t('info.fromToday')}>
                {d.fromToday === 0 ? t('info.isToday') : t(d.fromToday > 0 ? 'info.inDays' : 'info.daysAgo', { count: Math.abs(d.fromToday) })}
              </Row>
              <Row label={t('info.week')}>{t('info.weekValue', { year: d.isoYear, week: d.isoWeek })}</Row>
              <Row label={t('info.dayOfYear')}>{t('info.dayOfYearValue', { day: d.dayOfYear, total: d.daysInYear })}</Row>
              <Row label={t('info.quarter')}>{t('info.quarterValue', { quarter: d.quarter })}</Row>
              <Row label={t('info.leapYear')}>{t(d.leapYear ? 'info.yes' : 'info.no')}</Row>
              {d.lunar && (
                <>
                  <Row label={t('info.ganzhi')}>{t('info.ganzhiValue', { year: d.lunar.year, zodiac: t(`zodiac.${d.lunar.zodiac}`) })}</Row>
                  {d.term && (
                    <Row label={t('info.term')}>
                      {d.term.day === 0 ? t('info.termStarts', { term: t(`terms.${d.term.index}`) }) : t('info.termDay', { term: t(`terms.${d.term.index}`), day: d.term.day + 1 })}
                    </Row>
                  )}
                  <Row label={t('info.constellation')}>{t(`constellations.${d.constellation}`)}</Row>
                </>
              )}
            </div>
          </div>
        ) : null}
      </Panel>
    </div>
  )
}
