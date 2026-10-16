import { Badge, Button, Empty, Input, Panel, Select, Spinner } from '@toolforge/ui'
import { ValueRow, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { CalendarClock, Clock } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { formatRelative } from './relative'
import { UNITS, type Converted, type Unit } from './types'

interface Props {
  value: string
  onValueChange: (value: string) => void
  unit: Unit
  onUnitChange: (unit: Unit) => void
  compare: string
  onCompareChange: (zone: string) => void
  zones: string[]
  onNow: () => void
}

export function ToDate({ value, onValueChange, unit, onUnitChange, compare, onCompareChange, zones, onNow }: Props) {
  const { t, errorMessage } = usePlugin()
  const { i18n } = useTranslation()
  const trimmed = value.trim()
  const args = trimmed ? { value: trimmed, unit, timezones: [compare] } : null
  const { result, error, pending } = useDebouncedCall<Converted>(args ? 'from_timestamp' : null, args, [trimmed, unit, compare])

  return (
    <Panel
      icon={<Clock />}
      title={t('toDate.title')}
      extra={pending && <Spinner className="size-3.5 text-fg-subtle" />}
      bodyClassName="flex flex-col gap-3 overflow-auto p-4"
    >
      <div className="flex shrink-0 gap-2">
        <Input
          value={value}
          onChange={(event) => onValueChange(event.target.value)}
          placeholder="1700000000"
          className="font-mono"
          wrapperClassName="flex-1"
          aria-label={t('toDate.input')}
        />
        <Select<Unit>
          value={unit}
          onValueChange={onUnitChange}
          className="w-28"
          aria-label={t('toDate.unit')}
          options={UNITS.map((u) => ({ value: u, label: t(`units.${u}`) }))}
        />
        <Button onClick={onNow}>{t('toDate.now')}</Button>
      </div>
      <div className="flex shrink-0 items-center gap-2 text-xs text-fg-muted">
        {t('toDate.compare')}
        <Select<string>
          size="sm"
          value={compare}
          onValueChange={onCompareChange}
          className="w-56"
          aria-label={t('toDate.compare')}
          options={zones.map((zone) => ({ value: zone, label: zone }))}
        />
      </div>

      {!trimmed ? (
        <Empty icon={<CalendarClock />} title={t('toDate.empty')} />
      ) : error ? (
        <p className="text-[13px] text-danger">{errorMessage(error)}</p>
      ) : result ? (
        <div className="-mx-4 space-y-3">
          <div>
            {result.rows.map((row, index) => (
              <ValueRow
                key={row.timezone + index}
                labelWidth="w-36"
                label={
                  <span className="flex flex-col">
                    <span className="truncate text-fg">{index === 0 ? t('toDate.local') : row.timezone}</span>
                    <span className="font-normal text-fg-subtle">
                      {index === 0 ? `${row.timezone} ` : ''}
                      {row.offset}
                    </span>
                  </span>
                }
                value={row.datetime}
              />
            ))}
          </div>
          <div className="mx-4 h-px bg-border" />
          <div>
            <ValueRow labelWidth="w-36" label="ISO 8601" value={result.iso} />
            <ValueRow labelWidth="w-36" label="RFC 2822" value={result.rfc2822} />
            <ValueRow labelWidth="w-36" label={t('toDate.relative')} value={formatRelative(result.relativeSeconds, i18n.language)} />
            <div className="flex flex-wrap gap-1.5 px-3 pt-2">
              <Badge variant="primary">{t('toDate.detected', { unit: t(`units.${result.unit}`) })}</Badge>
              <Badge>{t(`weekdays.${result.weekday}`)}</Badge>
              <Badge>{t('toDate.dayOfYear', { day: result.dayOfYear })}</Badge>
              <Badge>{t('toDate.isoWeek', { week: result.isoWeek })}</Badge>
            </div>
          </div>
        </div>
      ) : null}
    </Panel>
  )
}
