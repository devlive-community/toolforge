import { useState } from 'react'
import { Button, Checkbox, CodeEditor, Input, Select } from '@toolforge/ui'
import { CopyButton, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Shuffle } from 'lucide-react'
import type { LinesOptions, LinesResult, Sort } from './types'

const TOGGLES: (keyof Pick<LinesOptions, 'trim' | 'removeEmpty' | 'dedupe' | 'ignoreCase' | 'reverse' | 'shuffle' | 'number'>)[] = [
  'trim',
  'removeEmpty',
  'dedupe',
  'ignoreCase',
  'reverse',
  'shuffle',
  'number',
]

export function LinesView({ input }: { input: string }) {
  const { t } = usePlugin()
  const [options, setOptions] = useState<LinesOptions>({
    trim: true,
    removeEmpty: true,
    dedupe: true,
    ignoreCase: false,
    sort: 'none',
    reverse: false,
    shuffle: false,
    number: false,
    prefix: '',
    suffix: '',
  })
  const [nonce, setNonce] = useState(0)
  const set = (patch: Partial<LinesOptions>) => setOptions((prev) => ({ ...prev, ...patch }))
  const { result } = useDebouncedCall<LinesResult>('lines', { input, ...options }, [input, ...Object.values(options), nonce])

  return (
    <div className="flex h-full flex-col">
      <div className="shrink-0 space-y-3 border-b border-border p-3">
        <div className="flex flex-wrap gap-x-4 gap-y-2">
          {TOGGLES.map((key) => (
            <Checkbox key={key} checked={options[key]} onCheckedChange={(value) => set({ [key]: value })}>
              {t(`lines.${key}`)}
            </Checkbox>
          ))}
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Select<Sort>
            size="sm"
            value={options.sort}
            onValueChange={(sort) => set({ sort })}
            className="w-40"
            aria-label={t('lines.sort')}
            options={(['none', 'asc', 'desc', 'natural', 'length'] as Sort[]).map((value) => ({ value, label: t(`sorts.${value}`) }))}
          />
          <Input size="sm" value={options.prefix} onChange={(e) => set({ prefix: e.target.value })} placeholder={t('lines.prefix')} wrapperClassName="w-28" aria-label={t('lines.prefix')} />
          <Input size="sm" value={options.suffix} onChange={(e) => set({ suffix: e.target.value })} placeholder={t('lines.suffix')} wrapperClassName="w-28" aria-label={t('lines.suffix')} />
          {options.shuffle && (
            <Button size="sm" onClick={() => setNonce((n) => n + 1)}>
              <Shuffle />
              {t('lines.reshuffle')}
            </Button>
          )}
          {result && <CopyButton text={result.output} label={t('common:copy')} variant="outline" className="ml-auto" />}
        </div>
      </div>
      <div className="min-h-0 flex-1">
        <CodeEditor value={result?.output ?? ''} readOnly language="text" aria-label={t('tabs.lines')} />
      </div>
      {result && (
        <p className="shrink-0 border-t border-border bg-surface-2 px-3 py-2 text-xs text-fg-muted">
          {t('lines.summary', { before: result.before, after: result.after, duplicates: result.duplicates, empty: result.empty })}
        </p>
      )}
    </div>
  )
}
