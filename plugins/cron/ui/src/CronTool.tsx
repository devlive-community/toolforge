import { useState } from 'react'
import { Badge, Empty, Input, Panel, Select, Spinner, Switch, cn } from '@toolforge/ui'
import { CopyButton, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { CalendarClock, History, ListChecks, Timer } from 'lucide-react'
import type { Field, FieldInfo, Part, Report, Run } from './types'

type TFunction = ReturnType<typeof usePlugin>['t']

const PRESETS = ['*/5 * * * *', '0 * * * *', '0 0 * * *', '30 9 * * MON-FRI', '0 0 1 * *', '0 0 L * *', '0 0 * * SUN', '*/10 * * * * *']

/** 数值按字段转成可读的名称（月份、星期） */
function valueLabel(t: TFunction, field: Field, value: number) {
  if (field === 'month') return t(`months.${value}`)
  if (field === 'dayOfWeek') return t(`weekdays.${value % 7}`)
  return String(value)
}

function describePart(t: TFunction, field: Field, part: Part) {
  const unit = t(`units.${field}`)
  const v = (value: number) => valueLabel(t, field, value)
  switch (part.kind) {
    case 'any':
      return t('parts.any', { unit })
    case 'value':
      return t('parts.value', { unit, value: v(part.value) })
    case 'range':
      return t('parts.range', { unit, from: v(part.from), to: v(part.to) })
    case 'step':
      if (part.from !== null && part.to !== null) return t('parts.stepRange', { unit, step: part.step, from: v(part.from), to: v(part.to) })
      if (part.from !== null) return t('parts.stepFrom', { unit, step: part.step, from: v(part.from) })
      return t('parts.step', { unit, step: part.step })
    case 'lastDay':
      return t('parts.lastDay')
    case 'lastWeekday':
      return t('parts.lastWeekday')
    case 'nearestWeekday':
      return t('parts.nearestWeekday', { day: part.day })
    case 'lastOfMonth':
      return t('parts.lastOfMonth', { weekday: t(`weekdays.${part.weekday}`) })
    case 'nth':
      return t('parts.nth', { weekday: t(`weekdays.${part.weekday}`), nth: part.nth })
  }
}

/** 相对时间：纯展示格式化 */
function relative(t: TFunction, seconds: number) {
  const abs = Math.abs(seconds)
  const days = Math.floor(abs / 86400)
  const hours = Math.floor((abs % 86400) / 3600)
  const minutes = Math.floor((abs % 3600) / 60)
  const secs = abs % 60
  const span =
    days > 0
      ? t('span.dh', { d: days, h: hours })
      : hours > 0
        ? t('span.hm', { h: hours, m: minutes })
        : minutes > 0
          ? t('span.ms', { m: minutes, s: secs })
          : t('span.s', { s: secs })
  return seconds >= 0 ? t('span.in', { span }) : t('span.ago', { span })
}

function FieldCard({ info }: { info: FieldInfo }) {
  const { t } = usePlugin()
  return (
    <div className="min-w-0 flex-1 rounded-card border border-border bg-surface p-3 text-center">
      <p className="truncate font-mono text-lg font-semibold text-primary" data-selectable>
        {info.raw}
      </p>
      <p className="mt-0.5 text-[11px] font-medium tracking-wide text-fg-subtle uppercase">{t(`fields.${info.field}`)}</p>
      <p className="mt-2 text-xs leading-relaxed text-fg-muted">{info.parts.map((part) => describePart(t, info.field, part)).join(t('parts.separator'))}</p>
    </div>
  )
}

function RunRow({ run, index }: { run: Run; index: number | null }) {
  const { t } = usePlugin()
  return (
    <li className="group flex items-center gap-3 rounded-control px-3 py-2 hover:bg-hover">
      <span className="w-6 shrink-0 text-right text-xs text-fg-subtle tabular-nums">{index ?? '−'}</span>
      <span className="font-mono text-[13px] text-fg tabular-nums" data-selectable>
        {run.local}
      </span>
      <span className="w-14 text-xs text-fg-muted">{t(`weekdays.${run.weekday}`)}</span>
      <span className="ml-auto text-xs text-fg-subtle tabular-nums">{relative(t, run.inSeconds)}</span>
      <CopyButton text={run.local} className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100" />
    </li>
  )
}

export function CronTool() {
  const { t, errorMessage } = usePlugin()
  const [expression, setExpression] = useState('30 9 * * MON-FRI')
  const [timezone, setTimezone] = useState('')
  const [domAndDow, setDomAndDow] = useState(false)

  const zones = useDebouncedCall<string[]>('timezones', {}, [])
  const trimmed = expression.trim()
  const args = trimmed ? { expression: trimmed, timezone, count: 12, domAndDow } : null
  const { result, error, pending } = useDebouncedCall<Report>(args ? 'evaluate' : null, args, [trimmed, timezone, domAndDow])
  const report = error ? null : result

  return (
    <div className="grid h-full min-h-0 grid-rows-[auto_auto_minmax(0,1fr)] gap-3">
      <section className="space-y-3 rounded-card border border-border bg-surface p-4 shadow-card">
        <div className="flex flex-wrap items-center gap-3">
          <Input
            value={expression}
            onChange={(event) => setExpression(event.target.value)}
            size="lg"
            className="font-mono"
            wrapperClassName="min-w-72 flex-1"
            leading={<Timer />}
            trailing={pending && <Spinner className="size-3.5" />}
            invalid={!!error}
            placeholder={t('input.placeholder')}
            aria-label={t('input.label')}
            spellCheck={false}
          />
          <Select<string>
            value={timezone}
            onValueChange={setTimezone}
            className="w-60"
            aria-label={t('input.timezone')}
            options={[{ value: '', label: t('input.systemZone') }, ...(zones.result ?? []).map((zone) => ({ value: zone, label: zone }))]}
          />
          <label className="flex items-center gap-2 text-[13px] text-fg">
            <Switch size="sm" checked={domAndDow} onCheckedChange={setDomAndDow} aria-label={t('input.domAndDow')} />
            {t('input.domAndDow')}
          </label>
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="mr-1 text-xs text-fg-subtle">{t('input.presets')}</span>
          {PRESETS.map((preset) => (
            <button
              key={preset}
              type="button"
              onClick={() => setExpression(preset)}
              className={cn(
                'rounded-control border px-2 py-0.5 font-mono text-[11px] outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring',
                preset === trimmed ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg-muted hover:bg-hover hover:text-fg',
              )}
            >
              {preset}
            </button>
          ))}
        </div>
        {error && <p className="text-xs text-danger">{errorMessage(error)}</p>}
      </section>

      {report ? (
        <div className="flex gap-2">
          {report.fields.map((info) => (
            <FieldCard key={info.field} info={info} />
          ))}
        </div>
      ) : (
        <div />
      )}

      <div className="grid min-h-0 grid-cols-[minmax(0,1.3fr)_minmax(0,1fr)] gap-3">
        <Panel
          icon={<CalendarClock />}
          title={t('runs.title')}
          extra={report && <Badge>{report.timezone}</Badge>}
          actions={report && <CopyButton text={report.next.map((r) => r.local).join('\n')} label={t('runs.copyAll')} variant="outline" />}
          bodyClassName="overflow-auto p-2"
        >
          {!report ? (
            <Empty icon={<CalendarClock />} title={t('runs.empty')} />
          ) : (
            <ol>
              {report.next.map((run, index) => (
                <RunRow key={run.iso} run={run} index={index + 1} />
              ))}
            </ol>
          )}
        </Panel>
        <div className="grid min-h-0 auto-rows-min gap-3">
          <Panel icon={<History />} title={t('runs.previous')} bodyClassName="p-2">
            {report?.previous ? <RunRow run={report.previous} index={null} /> : <p className="p-3 text-xs text-fg-subtle">{t('runs.none')}</p>}
          </Panel>
          <Panel icon={<ListChecks />} title={t('syntax.title')} bodyClassName="p-3">
            <dl className="grid grid-cols-[4.5rem_minmax(0,1fr)] gap-x-3 gap-y-1.5 text-xs">
              {(['star', 'list', 'range', 'step', 'last', 'weekday', 'nth', 'alias'] as const).map((key) => (
                <div key={key} className="contents">
                  <dt className="font-mono text-fg">{t(`syntax.${key}.token`)}</dt>
                  <dd className="text-fg-muted">{t(`syntax.${key}.text`)}</dd>
                </div>
              ))}
            </dl>
            {report && <p className="mt-3 text-[11px] text-fg-subtle">{t(report.hasSeconds ? 'syntax.withSeconds' : 'syntax.fiveFields')}</p>}
          </Panel>
        </div>
      </div>
    </div>
  )
}
