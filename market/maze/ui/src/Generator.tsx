import { useState } from 'react'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { Button, Input, Panel, Select, SegmentedControl } from '@toolforge/ui'
import { Import, Sparkles } from 'lucide-react'
import { draftFor, type Draft, type Op, type Side, type Spec } from './types'

export function Generator({ spec, disabled, generate }: { spec?: Spec; disabled: boolean; generate: (op: Op) => void }) {
  const { t } = usePlugin()
  const [draft, setDraft] = useState<Draft>(() => draftFor(spec))
  const [code, setCode] = useState('')
  const [mode, setMode] = useState<'new' | 'code'>('new')
  const update = (patch: Partial<Draft>) => setDraft({ ...draft, ...patch })
  return (
    <Panel icon={<Sparkles />} title={t('generate.title')} bodyClassName="flex h-full min-h-0 flex-col gap-3 p-3">
      <SegmentedControl
        size="sm"
        value={mode}
        onValueChange={setMode}
        aria-label={t('generate.mode')}
        options={[
          { value: 'new', label: t('generate.new') },
          { value: 'code', label: t('generate.code') },
        ]}
      />
      <div className="min-h-0 flex-1 space-y-3 overflow-auto">
        {mode === 'new' ? (
          <>
            <div className="flex flex-wrap gap-1">
              {[6, 12, 20].map((n) => (
                <Button
                  key={n}
                  size="sm"
                  disabled={disabled}
                  onClick={() =>
                    update({ rows: String(n), cols: String(n), entranceSide: 'left', entranceIndex: '1', exitSide: 'right', exitIndex: String(n) })
                  }
                >
                  {t('generate.preset', { value: n })}
                </Button>
              ))}
            </div>
            <div className="grid grid-cols-2 gap-2">
              {(['rows', 'cols'] as const).map((key) => (
                <label key={key} className="space-y-1 text-xs text-fg-muted">
                  <span>{t(`generate.${key}`)}</span>
                  <Input
                    size="sm"
                    type="number"
                    min={2}
                    max={40}
                    value={draft[key]}
                    disabled={disabled}
                    aria-label={t(`generate.${key}`)}
                    onChange={(e) => update({ [key]: e.target.value })}
                  />
                </label>
              ))}
            </div>
            {(['entrance', 'exit'] as const).map((key) => (
              <section key={key} className="space-y-2 rounded-control border border-border p-2.5">
                <p className="text-xs font-medium text-fg">{t(`generate.${key}`)}</p>
                <Select<Side>
                  size="sm"
                  disabled={disabled}
                  value={draft[`${key}Side`]}
                  onValueChange={(side) => update({ [`${key}Side`]: side })}
                  aria-label={t(`generate.${key}Side`)}
                  options={(['top', 'right', 'bottom', 'left'] as const).map((value) => ({ value, label: t(`side.${value}`) }))}
                />
                <Input
                  size="sm"
                  type="number"
                  min={1}
                  max={40}
                  disabled={disabled}
                  value={draft[`${key}Index`]}
                  aria-label={t(`generate.${key}Index`)}
                  onChange={(e) => update({ [`${key}Index`]: e.target.value })}
                />
                <p className="text-[10px] leading-relaxed text-fg-subtle">{t('generate.portHint')}</p>
              </section>
            ))}
            <label className="block space-y-1 text-xs text-fg-muted">
              <span>{t('generate.seed')}</span>
              <Input
                size="sm"
                disabled={disabled}
                value={draft.seed}
                maxLength={80}
                aria-label={t('generate.seed')}
                placeholder={t('generate.seedHint')}
                onChange={(e) => update({ seed: e.target.value })}
              />
            </label>
          </>
        ) : (
          <div className="space-y-3">
            <p className="text-xs leading-relaxed text-fg-muted">{t('generate.codeHint')}</p>
            <Input
              size="sm"
              className="font-mono"
              value={code}
              maxLength={256}
              disabled={disabled}
              aria-label={t('generate.code')}
              placeholder={t('generate.codePlaceholder')}
              onChange={(e) => setCode(e.target.value)}
            />
          </div>
        )}
        <p className="text-[11px] leading-relaxed text-fg-subtle">{t('generate.limits')}</p>
      </div>
      <Button
        size="sm"
        variant="primary"
        disabled={disabled || (mode === 'code' && !code.trim())}
        onClick={() => generate(mode === 'new' ? { type: 'new', draft, confirm: true } : { type: 'importCode', code, confirm: true })}
      >
        {mode === 'new' ? <Sparkles /> : <Import />}
        {t(mode === 'new' ? 'generate.start' : 'generate.import')}
      </Button>
    </Panel>
  )
}
