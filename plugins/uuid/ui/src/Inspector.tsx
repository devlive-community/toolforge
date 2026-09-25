import { useState } from 'react'
import { Empty, Input, Panel } from '@toolforge/ui'
import { ValueRow, useDebouncedCall, useLaunchInput, usePlugin } from '@toolforge/plugin-ui-sdk'
import { ScanSearch } from 'lucide-react'
import type { InspectResult } from './types'

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
