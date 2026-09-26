import { Button, Input, NumberInput, Select, Tooltip } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { ArrowDown, ArrowUp, X } from 'lucide-react'
import { KINDS, type Field, type Kind } from './types'

const RANGE_KINDS: Kind[] = ['integer', 'float', 'age']
const DATE_KINDS: Kind[] = ['date', 'datetime', 'timestamp']

interface FieldRowProps {
  field: Field
  first: boolean
  last: boolean
  onChange: (field: Field) => void
  onMove: (delta: number) => void
  onRemove: () => void
}

function NumberBox({ label, value, onChange }: { label: string; value: number | undefined; onChange: (value: number) => void }) {
  return (
    <label className="flex items-center gap-1.5 text-xs text-fg-muted">
      {label}
      <NumberInput size="sm" value={value ?? 0} onValueChange={onChange} min={-1_000_000_000} max={1_000_000_000} className="w-28" aria-label={label} />
    </label>
  )
}

export function FieldRow({ field, first, last, onChange, onMove, onRemove }: FieldRowProps) {
  const { t } = usePlugin()
  const set = (patch: Partial<Field>) => onChange({ ...field, ...patch })

  return (
    <div className="group space-y-2 rounded-control border border-border bg-surface p-2.5 hover:border-border-strong">
      <div className="flex items-center gap-2">
        <Input
          size="sm"
          value={field.name}
          onChange={(event) => set({ name: event.target.value })}
          className="font-mono"
          wrapperClassName="w-36 shrink-0"
          aria-label={t('fields.name')}
          spellCheck={false}
        />
        <Select<Kind>
          size="sm"
          value={field.kind}
          onValueChange={(kind) => set({ kind })}
          className="min-w-0 flex-1"
          aria-label={t('fields.kind')}
          options={KINDS.map((kind) => ({ value: kind, label: t(`kinds.${kind}`) }))}
        />
        <Tooltip content={t('fields.nullHint')}>
          <label className="flex shrink-0 items-center gap-1 text-[11px] text-fg-subtle">
            {t('fields.null')}
            <NumberInput size="sm" value={field.nullPercent} onValueChange={(nullPercent) => set({ nullPercent })} min={0} max={100} step={5} className="w-24" aria-label={t('fields.null')} />
          </label>
        </Tooltip>
        <div className="flex shrink-0 items-center opacity-60 group-hover:opacity-100">
          <Button size="icon-sm" variant="ghost" aria-label={t('fields.up')} disabled={first} onClick={() => onMove(-1)}>
            <ArrowUp />
          </Button>
          <Button size="icon-sm" variant="ghost" aria-label={t('fields.down')} disabled={last} onClick={() => onMove(1)}>
            <ArrowDown />
          </Button>
          <Button size="icon-sm" variant="ghost" aria-label={t('fields.remove')} className="hover:text-danger" onClick={onRemove}>
            <X />
          </Button>
        </div>
      </div>
      {(RANGE_KINDS.includes(field.kind) || field.kind === 'id' || DATE_KINDS.includes(field.kind) || field.kind === 'enum' || field.kind === 'constant') && (
        <div className="flex flex-wrap items-center gap-3 pl-1">
          {field.kind === 'id' && <NumberBox label={t('fields.start')} value={field.min ?? 1} onChange={(min) => set({ min })} />}
          {RANGE_KINDS.includes(field.kind) && (
            <>
              <NumberBox label={t('fields.min')} value={field.min ?? (field.kind === 'age' ? 18 : 0)} onChange={(min) => set({ min })} />
              <NumberBox label={t('fields.max')} value={field.max ?? (field.kind === 'age' ? 60 : 1000)} onChange={(max) => set({ max })} />
            </>
          )}
          {field.kind === 'float' && (
            <label className="flex items-center gap-1.5 text-xs text-fg-muted">
              {t('fields.decimals')}
              <NumberInput size="sm" value={field.decimals ?? 2} onValueChange={(decimals) => set({ decimals })} min={0} max={10} className="w-20" aria-label={t('fields.decimals')} />
            </label>
          )}
          {DATE_KINDS.includes(field.kind) && (
            <>
              <label className="flex items-center gap-1.5 text-xs text-fg-muted">
                {t('fields.from')}
                <Input size="sm" value={field.from ?? ''} onChange={(event) => set({ from: event.target.value })} placeholder="YYYY-MM-DD" className="font-mono" wrapperClassName="w-32" aria-label={t('fields.from')} />
              </label>
              <label className="flex items-center gap-1.5 text-xs text-fg-muted">
                {t('fields.to')}
                <Input size="sm" value={field.to ?? ''} onChange={(event) => set({ to: event.target.value })} placeholder="YYYY-MM-DD" className="font-mono" wrapperClassName="w-32" aria-label={t('fields.to')} />
              </label>
            </>
          )}
          {(field.kind === 'enum' || field.kind === 'constant') && (
            <Input
              size="sm"
              value={field.options ?? ''}
              onChange={(event) => set({ options: event.target.value })}
              placeholder={t(field.kind === 'enum' ? 'fields.optionsHint' : 'fields.constantHint')}
              wrapperClassName="min-w-0 flex-1"
              aria-label={t(field.kind === 'enum' ? 'fields.options' : 'fields.constant')}
            />
          )}
        </div>
      )}
    </div>
  )
}
