import { useState } from 'react'
import { Badge, Empty, Input, Panel } from '@toolforge/ui'
import { ValueRow, useDebouncedCall, useLaunchInput, usePlugin } from '@toolforge/plugin-ui-sdk'
import { ScanSearch } from 'lucide-react'
import type { InspectResult, SnowflakeDecoded } from './types'

const localTime = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'medium' })

export function Inspector() {
  const { t, errorMessage } = usePlugin()
  const [input, setInput] = useState('')
  useLaunchInput((text) => setInput(text))
  const trimmed = input.trim()
  const { result, error } = useDebouncedCall<InspectResult>(trimmed ? 'inspect' : null, trimmed ? { input: trimmed } : null, [trimmed])

  return (
    <Panel icon={<ScanSearch />} title={t('inspect.title')} bodyClassName="flex flex-col gap-2 overflow-auto p-3">
      <Input value={input} onChange={(event) => setInput(event.target.value)} placeholder={t('inspect.placeholder')} className="font-mono" wrapperClassName="shrink-0" aria-label={t('inspect.title')} />
      {!trimmed ? (
        <Empty className="p-4" icon={<ScanSearch />} title={t('inspect.hint')} />
      ) : error ? (
        <p className="px-1 text-xs text-danger">{errorMessage(error)}</p>
      ) : result?.snowflake ? (
        <div className="-mx-3">
          <ValueRow label={t('inspect.kind')} value={t('kinds.snowflake')} />
          <ValueRow label={t('inspect.hex')} value={result.hex} />
          <p className="px-3 pt-1 text-[11px] text-fg-subtle">{t('snowflake.decodeHint')}</p>
          <div className="mt-1 space-y-1.5 px-3">
            {result.snowflake.map((item) => (
              <SnowflakeRow key={item.preset} item={item} />
            ))}
          </div>
        </div>
      ) : result ? (
        <div className="-mx-3">
          <ValueRow label={t('inspect.kind')} value={result.kind === 'ulid' ? 'ULID' : result.version !== null ? `UUID v${result.version}` : 'UUID'} />
          <ValueRow label={t('inspect.canonical')} value={result.canonical} />
          {result.variant && <ValueRow label={t('inspect.variant')} value={t(`variants.${result.variant}`)} />}
          {result.unixMs !== null && (
            <>
              <ValueRow label={t('inspect.local')} value={localTime.format(result.unixMs)} />
              <ValueRow label={t('inspect.utc')} value={result.timestamp ?? ''} />
              <ValueRow label={t('inspect.unixMs')} value={String(result.unixMs)} />
            </>
          )}
          <ValueRow label={t('inspect.hex')} value={result.hex} />
        </div>
      ) : null}
    </Panel>
  )
}

function SnowflakeRow({ item }: { item: SnowflakeDecoded }) {
  const { t } = usePlugin()
  const parts = [
    item.datacenterId !== null && t('snowflake.part.datacenter', { value: item.datacenterId }),
    t('snowflake.part.worker', { value: item.workerId }),
    t('snowflake.part.sequence', { value: item.sequence }),
  ].filter(Boolean)
  return (
    <div className={`rounded-control border border-border px-2.5 py-2 text-xs ${item.plausible ? '' : 'opacity-60'}`}>
      <div className="flex items-center gap-2">
        <span className="font-medium text-fg">{t(`snowflake.presets.${item.preset}`)}</span>
        {item.plausible && <Badge variant="success">{t('snowflake.plausible')}</Badge>}
      </div>
      <p className="mt-1 font-mono text-[12px] text-fg" data-selectable>
        {item.timestamp ? localTime.format(item.unixMs) : t('snowflake.outOfRange')}
      </p>
      <p className="mt-0.5 font-mono text-[11px] text-fg-muted" data-selectable>
        {parts.join(' · ')}
      </p>
    </div>
  )
}
