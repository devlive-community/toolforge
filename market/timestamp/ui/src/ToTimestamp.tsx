import { useState } from 'react'
import { Badge, Empty, Input, Panel, Select, Spinner, cn } from '@toolforge/ui'
import { ValueRow, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { CalendarDays, Hash } from 'lucide-react'
import type { Parsed } from './types'

const EXAMPLES = ['2024-02-29 12:00:00', '2024/2/29', '2023-11-14T22:13:20Z', '2023-11-15T06:13:20+08:00', 'Tue, 14 Nov 2023 22:13:20 +0000']
const NATURAL_EXAMPLES = ['明天上午 9 点', '下周五 15:00', '3 天后', '半小时后', 'next friday 9am', 'in 2 hours']

export function ToTimestamp({ zones, local, initial }: { zones: string[]; local: string; initial?: string }) {
  const { t, errorMessage } = usePlugin()
  const [input, setInput] = useState(initial ?? '2024-02-29 12:00:00')
  const [timezone, setTimezone] = useState('local')
  // 自然语言有多种解释时选中的候选；输入变化后回到第一个
  const [choice, setChoice] = useState({ input: '', index: 0 })
  const trimmed = input.trim()
  const index = choice.input === trimmed ? choice.index : 0
  const args = trimmed ? { input: trimmed, timezone, choice: index } : null
  const { result, error, pending } = useDebouncedCall<Parsed>(args ? 'to_timestamp' : null, args, [trimmed, timezone, index])

  return (
    <Panel
      icon={<Hash />}
      title={t('toTimestamp.title')}
      extra={pending && <Spinner className="size-3.5 text-fg-subtle" />}
      bodyClassName="flex flex-col gap-3 overflow-auto p-4"
    >
      <Input
        value={input}
        onChange={(event) => setInput(event.target.value)}
        placeholder="2024-02-29 12:00:00"
        className="font-mono"
        wrapperClassName="shrink-0"
        aria-label={t('toTimestamp.input')}
      />
      <div className="flex shrink-0 items-center gap-2 text-xs text-fg-muted">
        {t('toTimestamp.timezone')}
        <Select<string>
          size="sm"
          value={timezone}
          onValueChange={setTimezone}
          className="w-56"
          aria-label={t('toTimestamp.timezone')}
          options={[{ value: 'local', label: t('toTimestamp.localZone', { zone: local }) }, ...zones.map((zone) => ({ value: zone, label: zone }))]}
        />
      </div>

      {!trimmed ? (
        <Empty icon={<CalendarDays />} title={t('toTimestamp.empty')} />
      ) : error ? (
        <p className="text-[13px] text-danger">{errorMessage(error)}</p>
      ) : result ? (
        <div className="-mx-4">
          <ValueRow labelWidth="w-36" label={t('units.s')} value={String(result.seconds)} />
          <ValueRow labelWidth="w-36" label={t('units.ms')} value={String(result.milliseconds)} />
          <ValueRow labelWidth="w-36" label={t('units.us')} value={String(result.microseconds)} />
          <ValueRow labelWidth="w-36" label={t('units.ns')} value={result.nanoseconds} />
          <ValueRow labelWidth="w-36" label="ISO 8601" value={result.iso} />
          <div className="flex flex-wrap gap-1.5 px-3 pt-2">
            <Badge variant="primary">{t(`formats.${result.format}`)}</Badge>
            <Badge>
              {result.zoned.timezone} {result.zoned.offset}
            </Badge>
          </div>
          {result.candidates.length > 0 && (
            <div className="space-y-1.5 px-3 pt-3">
              <p className="text-xs text-fg-muted">{t(result.candidates.length > 1 ? 'natural.pick' : 'natural.understood')}</p>
              <div className="flex flex-wrap gap-1.5">
                {result.candidates.map((candidate, i) => (
                  <button
                    key={`${candidate.datetime}-${i}`}
                    type="button"
                    onClick={() => setChoice({ input: trimmed, index: i })}
                    className={cn(
                      'rounded-control border px-2.5 py-1.5 text-left text-xs outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring',
                      i === result.choice ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg hover:bg-hover',
                    )}
                  >
                    <span className="block font-mono tabular-nums">
                      {candidate.datetime} {t(`weekdays.${candidate.weekday}`)}
                    </span>
                    {candidate.notes.length > 0 && (
                      <span className="block text-[11px] text-fg-muted">{candidate.notes.map((note) => t(`natural.notes.${note}`)).join(t('natural.separator'))}</span>
                    )}
                  </button>
                ))}
              </div>
            </div>
          )}
        </div>
      ) : null}

      <div className="mt-auto shrink-0 rounded-control bg-surface-2 p-3 text-xs text-fg-muted">
        <p className="font-medium text-fg">{t('toTimestamp.formats')}</p>
        <ul className="mt-1.5 space-y-0.5 font-mono" data-selectable>
          {EXAMPLES.map((example) => (
            <li key={example}>
              <button type="button" className="text-left hover:text-primary-fg" onClick={() => setInput(example)}>
                {example}
              </button>
            </li>
          ))}
        </ul>
        <p className="mt-2.5 font-medium text-fg">{t('natural.examples')}</p>
        <ul className="mt-1.5 flex flex-wrap gap-x-3 gap-y-0.5 font-mono" data-selectable>
          {NATURAL_EXAMPLES.map((example) => (
            <li key={example}>
              <button type="button" className="text-left hover:text-primary-fg" onClick={() => setInput(example)}>
                {example}
              </button>
            </li>
          ))}
        </ul>
      </div>
    </Panel>
  )
}
