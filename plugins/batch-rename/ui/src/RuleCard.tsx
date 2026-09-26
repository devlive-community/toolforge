import type { ReactNode } from 'react'
import { Button, Input, NumberInput, SegmentedControl, Select, Switch, Tooltip, cn } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { ArrowDown, ArrowUp, CaseSensitive, Regex, Trash2 } from 'lucide-react'
import type { Part, Place, Rule, Step } from './types'

const TOKENS = ['{name}', '{ext}', '{n}', '{n:3}', '{parent}', '{date:%Y-%m-%d}', '{exif:%Y%m%d_%H%M%S}']

function Field({ label, children, className }: { label: string; children: ReactNode; className?: string }) {
  return (
    <label className={cn('flex min-w-0 flex-col gap-1', className)}>
      <span className="text-[11px] font-medium text-fg-muted">{label}</span>
      {children}
    </label>
  )
}

function Toggle({ label, checked, onChange }: { label: string; checked: boolean; onChange: (value: boolean) => void }) {
  return (
    <label className="flex items-center gap-2 text-xs text-fg">
      <Switch size="sm" checked={checked} onCheckedChange={onChange} aria-label={label} />
      {label}
    </label>
  )
}

interface RuleCardProps {
  step: Step
  first: boolean
  last: boolean
  onChange: (step: Step) => void
  onMove: (delta: number) => void
  onRemove: () => void
}

export function RuleCard({ step, first, last, onChange, onMove, onRemove }: RuleCardProps) {
  const { t } = usePlugin()
  const set = (patch: Partial<Rule>) => onChange({ ...step, ...patch } as Step)

  const partSelect = (part: Part) => (
    <Field label={t('fields.part')}>
      <Select<Part>
        size="sm"
        value={part}
        onValueChange={(value) => set({ part: value } as Partial<Rule>)}
        aria-label={t('fields.part')}
        options={(['stem', 'ext', 'full'] as Part[]).map((value) => ({ value, label: t(`parts.${value}`) }))}
      />
    </Field>
  )
  const placeControl = (place: Place, withIndex: boolean) => (
    <Field label={t('fields.place')}>
      <SegmentedControl<Place>
        size="sm"
        value={place}
        onValueChange={(value) => set({ place: value } as Partial<Rule>)}
        aria-label={t('fields.place')}
        options={(withIndex ? ['start', 'end', 'index'] : ['start', 'end']).map((value) => ({ value: value as Place, label: t(`places.${value}`) }))}
      />
    </Field>
  )

  let body: ReactNode = null
  switch (step.kind) {
    case 'replace':
      body = (
        <>
          <div className="grid grid-cols-2 gap-2">
            <Field label={t('fields.find')}>
              <Input size="sm" value={step.find} onChange={(e) => set({ find: e.target.value })} className="font-mono" spellCheck={false} />
            </Field>
            <Field label={t('fields.replace')}>
              <Input size="sm" value={step.replace} onChange={(e) => set({ replace: e.target.value })} className="font-mono" spellCheck={false} />
            </Field>
          </div>
          <div className="flex items-end gap-3">
            {partSelect(step.part)}
            <Tooltip content={t('fields.caseSensitive')}>
              <Button size="icon-sm" variant={step.caseSensitive ? 'primary' : 'ghost'} aria-pressed={step.caseSensitive} aria-label={t('fields.caseSensitive')} onClick={() => set({ caseSensitive: !step.caseSensitive })}>
                <CaseSensitive />
              </Button>
            </Tooltip>
            <Tooltip content={t('fields.regex')}>
              <Button size="icon-sm" variant={step.regex ? 'primary' : 'ghost'} aria-pressed={step.regex} aria-label={t('fields.regex')} onClick={() => set({ regex: !step.regex })}>
                <Regex />
              </Button>
            </Tooltip>
          </div>
        </>
      )
      break
    case 'insert':
      body = (
        <>
          <Field label={t('fields.text')}>
            <Input size="sm" value={step.text} onChange={(e) => set({ text: e.target.value })} spellCheck={false} />
          </Field>
          <div className="flex flex-wrap items-end gap-2">
            {placeControl(step.place, true)}
            {step.place === 'index' && (
              <Field label={t('fields.index')} className="w-24">
                <NumberInput size="sm" value={step.index} onValueChange={(index) => set({ index })} min={0} max={255} aria-label={t('fields.index')} />
              </Field>
            )}
            {partSelect(step.part)}
          </div>
        </>
      )
      break
    case 'remove':
      body = (
        <div className="flex flex-wrap items-end gap-2">
          <Field label={t('fields.count')} className="w-24">
            <NumberInput size="sm" value={step.count} onValueChange={(count) => set({ count })} min={1} max={255} aria-label={t('fields.count')} />
          </Field>
          {placeControl(step.place, true)}
          {step.place === 'index' && (
            <Field label={t('fields.index')} className="w-24">
              <NumberInput size="sm" value={step.index} onValueChange={(index) => set({ index })} min={0} max={255} aria-label={t('fields.index')} />
            </Field>
          )}
          {partSelect(step.part)}
        </div>
      )
      break
    case 'case':
      body = (
        <div className="grid grid-cols-2 gap-2">
          <Field label={t('fields.mode')}>
            <Select
              size="sm"
              value={step.mode}
              onValueChange={(mode) => set({ mode })}
              aria-label={t('fields.mode')}
              options={(['lower', 'upper', 'title', 'sentence'] as const).map((value) => ({ value, label: t(`cases.${value}`) }))}
            />
          </Field>
          {partSelect(step.part)}
        </div>
      )
      break
    case 'number':
      body = (
        <>
          <div className="grid grid-cols-3 gap-2">
            <Field label={t('fields.start')}>
              <NumberInput size="sm" value={step.start} onValueChange={(start) => set({ start })} min={-99999} max={999999} aria-label={t('fields.start')} />
            </Field>
            <Field label={t('fields.step')}>
              <NumberInput size="sm" value={step.step} onValueChange={(value) => set({ step: value })} min={-1000} max={1000} aria-label={t('fields.step')} />
            </Field>
            <Field label={t('fields.pad')}>
              <NumberInput size="sm" value={step.pad} onValueChange={(pad) => set({ pad })} min={0} max={10} aria-label={t('fields.pad')} />
            </Field>
          </div>
          <div className="flex items-end gap-2">
            {placeControl(step.place, false)}
            <Field label={t('fields.separator')} className="w-24">
              <Input size="sm" value={step.separator} onChange={(e) => set({ separator: e.target.value })} className="font-mono" />
            </Field>
          </div>
        </>
      )
      break
    case 'template':
      body = (
        <>
          <Field label={t('fields.pattern')}>
            <Input size="sm" value={step.pattern} onChange={(e) => set({ pattern: e.target.value })} className="font-mono" spellCheck={false} />
          </Field>
          <div className="flex flex-wrap gap-1">
            {TOKENS.map((token) => (
              <button
                key={token}
                type="button"
                onClick={() => set({ pattern: step.pattern + token })}
                className="rounded-control border border-border px-1.5 py-0.5 font-mono text-[11px] text-fg-muted outline-none hover:bg-hover hover:text-fg focus-visible:ring-2 focus-visible:ring-ring"
              >
                {token}
              </button>
            ))}
          </div>
          <p className="text-[11px] leading-snug text-fg-subtle">{t('fields.templateHint')}</p>
        </>
      )
      break
    case 'extension':
      body = (
        <div className="grid grid-cols-2 gap-2">
          <Field label={t('fields.mode')}>
            <Select
              size="sm"
              value={step.mode}
              onValueChange={(mode) => set({ mode })}
              aria-label={t('fields.mode')}
              options={(['lower', 'upper', 'set', 'remove'] as const).map((value) => ({ value, label: t(`extensions.${value}`) }))}
            />
          </Field>
          {step.mode === 'set' && (
            <Field label={t('fields.extension')}>
              <Input size="sm" value={step.value} onChange={(e) => set({ value: e.target.value })} placeholder="jpg" className="font-mono" />
            </Field>
          )}
        </div>
      )
      break
    case 'clean':
      body = (
        <>
          <div className="flex flex-wrap gap-x-4 gap-y-2">
            <Toggle label={t('fields.collapseSpaces')} checked={step.collapseSpaces} onChange={(collapseSpaces) => set({ collapseSpaces })} />
            <Toggle label={t('fields.trim')} checked={step.trim} onChange={(trim) => set({ trim })} />
            <Toggle label={t('fields.removeIllegal')} checked={step.removeIllegal} onChange={(removeIllegal) => set({ removeIllegal })} />
          </div>
          <Field label={t('fields.spacesTo')}>
            <Select
              size="sm"
              value={step.spacesTo}
              onValueChange={(spacesTo) => set({ spacesTo })}
              aria-label={t('fields.spacesTo')}
              options={['', '_', '-', '.'].map((value) => ({ value, label: value ? `“${value}”` : t('fields.keepSpaces') }))}
            />
          </Field>
        </>
      )
      break
  }

  return (
    <section className={cn('space-y-2.5 rounded-card border border-border bg-surface p-2.5', !step.enabled && 'opacity-60')}>
      <header className="flex items-center gap-1.5">
        <Switch size="sm" checked={step.enabled} onCheckedChange={(enabled) => onChange({ ...step, enabled })} aria-label={t('rules.enabled')} />
        <span className="min-w-0 flex-1 truncate text-[13px] font-medium text-fg">{t(`kinds.${step.kind}`)}</span>
        <Button size="icon-sm" variant="ghost" aria-label={t('rules.up')} disabled={first} onClick={() => onMove(-1)}>
          <ArrowUp />
        </Button>
        <Button size="icon-sm" variant="ghost" aria-label={t('rules.down')} disabled={last} onClick={() => onMove(1)}>
          <ArrowDown />
        </Button>
        <Button size="icon-sm" variant="ghost" aria-label={t('rules.remove')} onClick={onRemove}>
          <Trash2 />
        </Button>
      </header>
      {body}
    </section>
  )
}
