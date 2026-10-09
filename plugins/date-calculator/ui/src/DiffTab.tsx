import { useState } from 'react'
import { Button, Checkbox, Panel } from '@toolforge/ui'
import { CopyButton, useDebouncedCall, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { ArrowDownUp, Briefcase, Hourglass } from 'lucide-react'
import { DateInput } from './DateInput'
import type { Difference } from './types'

function Stat({ label, value, hint }: { label: string; value: string; hint?: string }) {
  return (
    <div className="rounded-control border border-border px-3 py-2.5">
      <p className="text-[11px] text-fg-muted">{label}</p>
      <p className="mt-0.5 text-lg font-medium text-fg tabular-nums" data-selectable>
        {value}
      </p>
      {hint && <p className="text-[11px] text-fg-subtle">{hint}</p>}
    </div>
  )
}

export function DiffTab({ today }: { today: string }) {
  const { t, errorMessage } = usePlugin()
  const [start, setStart] = useState(today)
  const [end, setEnd] = usePluginState('diffEnd', '')
  const [includeEnd, setIncludeEnd] = usePluginState('includeEnd', false)
  const args = start.trim() && end.trim() ? { start, end, includeEnd } : null
  const diff = useDebouncedCall<Difference>(args ? 'diff' : null, args, [start, end, includeEnd])
  const r = diff.result
  const n = (value: number) => value.toLocaleString()

  return (
    <div className="grid h-full min-h-0 grid-cols-[320px_minmax(0,1fr)] gap-3">
      <Panel icon={<Hourglass />} title={t('diff.title')} bodyClassName="space-y-3 p-3">
        <DateInput label={t('diff.start')} value={start} onChange={setStart} today={today} />
        <div className="flex justify-center">
          <Button
            size="icon-sm"
            variant="ghost"
            aria-label={t('diff.swap')}
            onClick={() => {
              setStart(end)
              setEnd(start)
            }}
          >
            <ArrowDownUp />
          </Button>
        </div>
        <DateInput label={t('diff.end')} value={end} onChange={setEnd} today={today} />
        <Checkbox checked={includeEnd} onCheckedChange={setIncludeEnd}>
          <span className="text-[13px] text-fg">{t('diff.includeEnd')}</span>
        </Checkbox>
        <p className="text-[11px] leading-snug text-fg-subtle">{t('diff.hint')}</p>
      </Panel>

      <Panel title={t('diff.result')} bodyClassName="space-y-4 overflow-auto p-4">
        {!args ? (
          <p className="text-[13px] text-fg-subtle">{t('diff.empty')}</p>
        ) : diff.error ? (
          <p className="text-[13px] text-danger">{errorMessage(diff.error)}</p>
        ) : r ? (
          <>
            <div className="flex items-end gap-3">
              <p className="font-mono text-4xl font-semibold text-fg tabular-nums" data-selectable>
                {r.negative ? '−' : ''}
                {n(r.totalDays)}
              </p>
              <p className="pb-1 text-[13px] text-fg-muted">{t('diff.days', { count: r.totalDays })}</p>
              <CopyButton text={String(r.totalDays)} className="mb-0.5" />
              {r.negative && <p className="pb-1 text-[11px] text-fg-subtle">{t('diff.negative')}</p>}
            </div>
            <div className="grid grid-cols-3 gap-2">
              <Stat label={t('diff.ymd')} value={t('diff.ymdValue', { years: r.years, months: r.months, days: r.days })} />
              <Stat label={t('diff.weeks')} value={t('diff.weeksValue', { weeks: n(r.weeks), days: r.weekDays })} />
              {r.hasTime && <Stat label={t('diff.hms')} value={t('diff.hmsValue', { hours: r.hours, minutes: r.minutes, seconds: r.seconds })} />}
              <Stat label={t('diff.totalHours')} value={n(r.totalHours)} />
              <Stat label={t('diff.totalMinutes')} value={n(r.totalMinutes)} />
              <Stat label={t('diff.totalSeconds')} value={n(r.totalSeconds)} />
            </div>
            {r.workdays && (
              <div className="space-y-2">
                <p className="flex items-center gap-1.5 text-xs font-medium text-fg-muted">
                  <Briefcase className="size-3.5" />
                  {t('diff.workdays')}
                </p>
                <div className="grid grid-cols-2 gap-2">
                  <Stat label={t('diff.weekdays')} value={n(r.workdays.weekdays)} hint={t('diff.weekdaysHint')} />
                  <Stat
                    label={t('diff.china')}
                    value={n(r.workdays.china)}
                    hint={t('diff.chinaHint', { holidays: r.workdays.holidays, adjusted: r.workdays.adjusted })}
                  />
                </div>
                {r.workdays.beyondData && <p className="text-[11px] text-warning">{t('diff.beyondData', { year: r.workdays.dataUntil })}</p>}
              </div>
            )}
          </>
        ) : null}
      </Panel>
    </div>
  )
}
