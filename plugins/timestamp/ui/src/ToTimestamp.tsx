import { useState } from 'react'
import { Badge, Empty, Input, Panel, Select, Spinner } from '@toolforge/ui'
import { ValueRow, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { CalendarDays, Hash } from 'lucide-react'
import type { Parsed } from './types'

const EXAMPLES = ['2024-02-29 12:00:00', '2024/2/29', '2023-11-14T22:13:20Z', '2023-11-15T06:13:20+08:00', 'Tue, 14 Nov 2023 22:13:20 +0000']

export function ToTimestamp({ zones, local }: { zones: string[]; local: string }) {
  const { t, errorMessage } = usePlugin()
  const [input, setInput] = useState('2024-02-29 12:00:00')
  const [timezone, setTimezone] = useState('local')
  const trimmed = input.trim()
  const args = trimmed ? { input: trimmed, timezone } : null
  const { result, error, pending } = useDebouncedCall<Parsed>(args ? 'to_timestamp' : null, args, [trimmed, timezone])

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
      </div>
    </Panel>
  )
}
