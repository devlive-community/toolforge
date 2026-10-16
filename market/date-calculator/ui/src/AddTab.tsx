import { useState } from 'react'
import { NumberInput, Panel, SegmentedControl } from '@toolforge/ui'
import { CopyButton, useDebouncedCall, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { CalendarPlus } from 'lucide-react'
import { DateInput } from './DateInput'
import type { Added } from './types'

type Mode = 'calendar' | 'workdays'
type Sign = 'add' | 'subtract'
type Calendar = 'weekdays' | 'china'
const UNITS = ['years', 'months', 'weeks', 'days', 'hours', 'minutes'] as const
type Unit = (typeof UNITS)[number]

export function AddTab({ today }: { today: string }) {
  const { t, errorMessage } = usePlugin()
  const [start, setStart] = useState(today)
  const [mode, setMode] = usePluginState<Mode>('addMode', 'calendar')
  const [sign, setSign] = useState<Sign>('add')
  const [calendar, setCalendar] = usePluginState<Calendar>('workCalendar', 'china')
  const [amount, setAmount] = useState<Record<Unit, number>>({ years: 0, months: 0, weeks: 0, days: 30, hours: 0, minutes: 0 })
  const [workdays, setWorkdays] = useState(10)
  const k = sign === 'add' ? 1 : -1
  const signed = Object.fromEntries(UNITS.map((unit) => [unit, amount[unit] * k]))
  const fn = start.trim() ? (mode === 'calendar' ? 'add' : 'add_workdays') : null
  const args = mode === 'calendar' ? { start, ...signed } : { start, days: workdays * k, calendar }
  const result = useDebouncedCall<Added>(fn, args, [fn, start, sign, calendar, workdays, ...UNITS.map((u) => amount[u])])
  const r = result.result

  return (
    <div className="grid h-full min-h-0 grid-cols-[320px_minmax(0,1fr)] gap-3">
      <Panel icon={<CalendarPlus />} title={t('add.title')} bodyClassName="space-y-3 overflow-auto p-3">
        <DateInput label={t('add.start')} value={start} onChange={setStart} today={today} />
        <SegmentedControl<Mode>
          size="sm"
          value={mode}
          onValueChange={setMode}
          aria-label={t('add.mode')}
          options={(['calendar', 'workdays'] as Mode[]).map((value) => ({ value, label: t(`add.modes.${value}`) }))}
        />
        <SegmentedControl<Sign>
          size="sm"
          value={sign}
          onValueChange={setSign}
          aria-label={t('add.sign')}
          options={(['add', 'subtract'] as Sign[]).map((value) => ({ value, label: t(`add.signs.${value}`) }))}
        />
        {mode === 'calendar' ? (
          <div className="grid grid-cols-2 gap-2">
            {UNITS.map((unit) => (
              <label key={unit} className="space-y-1">
                <span className="block text-[11px] text-fg-muted">{t(`add.units.${unit}`)}</span>
                <NumberInput size="sm" value={amount[unit]} min={0} max={100000} onValueChange={(v) => setAmount((prev) => ({ ...prev, [unit]: v }))} aria-label={t(`add.units.${unit}`)} />
              </label>
            ))}
          </div>
        ) : (
          <div className="space-y-2">
            <label className="block space-y-1">
              <span className="block text-[11px] text-fg-muted">{t('add.workdays')}</span>
              <NumberInput size="sm" value={workdays} min={0} max={50000} onValueChange={setWorkdays} aria-label={t('add.workdays')} />
            </label>
            <SegmentedControl<Calendar>
              size="sm"
              value={calendar}
              onValueChange={setCalendar}
              aria-label={t('add.calendar')}
              options={(['china', 'weekdays'] as Calendar[]).map((value) => ({ value, label: t(`add.calendars.${value}`) }))}
            />
            <p className="text-[11px] leading-snug text-fg-subtle">{t('add.workdaysHint')}</p>
          </div>
        )}
      </Panel>

      <Panel title={t('add.result')} bodyClassName="p-4">
        {result.error ? (
          <p className="text-[13px] text-danger">{errorMessage(result.error)}</p>
        ) : r ? (
          <div className="space-y-2">
            <div className="flex items-center gap-3">
              <p className="font-mono text-4xl font-semibold text-fg tabular-nums" data-selectable>
                {r.result}
              </p>
              <CopyButton text={r.result} />
            </div>
            <p className="text-[13px] text-fg-muted">{t(`weekdays.${r.weekday}`)}</p>
            {r.clamped && <p className="text-[11px] text-warning">{t('add.clamped')}</p>}
          </div>
        ) : (
          <p className="text-[13px] text-fg-subtle">{t('add.empty')}</p>
        )}
      </Panel>
    </div>
  )
}
