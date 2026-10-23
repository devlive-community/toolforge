import { useState } from 'react'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Button, Checkbox, Input, Pagination, Select, Spinner, Tooltip } from '@toolforge/ui'
import { Plus, Trash2 } from 'lucide-react'
import type { Appliance, Page, Pricing, Tariff, Usage } from './types'
export function Pages({ data, change }: { data: Page<unknown>; change: (offset: number) => void }) {
  const { t } = usePlugin()
  return (
    <Pagination
      page={data.offset / data.limit + 1}
      pageSize={data.limit}
      total={data.total}
      onPageChange={(p) => change((p - 1) * data.limit)}
      prevLabel={t('page.prev')}
      nextLabel={t('page.next')}
      pageLabel={(n) => t('page.number', { n })}
      totalLabel={t('page.total', { n: data.total })}
    />
  )
}
export function Field({ label, value, change, maxLength = 32 }: { label: string; value: string; change: (value: string) => void; maxLength?: number }) {
  return (
    <label className="flex min-w-0 flex-col gap-1 text-[11px] text-fg-muted">
      <span>{label}</span>
      <Input size="sm" value={value} onChange={(e) => change(e.target.value)} aria-label={label} maxLength={maxLength} />
    </label>
  )
}
function UsageEditor({ usage, change, label }: { usage: Usage; change: (value: Usage) => void; label: string }) {
  const { t } = usePlugin()
  return (
    <fieldset className="min-w-0 space-y-2">
      <legend className="mb-2 text-[11px] font-medium text-fg-muted">{label}</legend>
      <div className="grid grid-cols-[minmax(0,1fr)_85px] items-end gap-2">
        <Field label={t('usage.power')} value={usage.power} change={(power) => change({ ...usage, power })} />
        <Select
          size="sm"
          aria-label={t('usage.unit')}
          value={usage.unit}
          onValueChange={(unit) => change({ ...usage, unit })}
          options={[
            { value: 'w', label: t('units.w') },
            { value: 'kw', label: t('units.kw') },
          ]}
        />
      </div>
      <div className="grid grid-cols-3 gap-2">
        {(['peak', 'shoulder', 'valley'] as const).map((key) => (
          <Field key={key} label={t(`usage.${key}`)} value={usage[key]} change={(value) => change({ ...usage, [key]: value })} />
        ))}
      </div>
    </fieldset>
  )
}
export function Appliances({
  values,
  change,
  add,
  disabled,
}: {
  values: Appliance[]
  change: (values: Appliance[]) => void
  add: (preset: string) => void
  disabled: boolean
}) {
  const { t, errorMessage } = usePlugin()
  const [offset, setOffset] = useState(0)
  const [preset, setPreset] = useState('custom')
  const ids = values.map((a) => a.id)
  const signature = ids.join('|')
  const { result, pending, error } = useDebouncedCall<Page<string>>('editPage', { ids, offset, limit: 3 }, [signature, offset])
  const update = (a: Appliance) => change(values.map((old) => (old.id === a.id ? a : old)))
  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-2">
        <Select
          size="sm"
          value={preset}
          onValueChange={setPreset}
          aria-label={t('appliances.presets')}
          options={['custom', 'lighting', 'computer', 'cooling', 'fridge'].map((value) => ({ value, label: t(`presets.${value}`) }))}
        />
        <Button
          size="sm"
          disabled={disabled || values.length >= 100}
          onClick={() => {
            add(preset)
            setOffset(0)
          }}
        >
          <Plus />
          {t('appliances.add')}
        </Button>
      </div>
      <p className="text-[11px] leading-relaxed text-fg-subtle">{t('appliances.assumptions')}</p>
      {error && (
        <p role="alert" className="text-xs text-danger">
          {errorMessage(error)}
        </p>
      )}
      {pending ? (
        <Spinner className="size-4" />
      ) : (
        result?.items.map((id) => {
          const a = values.find((value) => value.id === id)
          if (!a) return null
          return (
            <article key={a.id} className="space-y-3 rounded-card border border-border bg-surface-2/50 p-3">
              <div className="grid grid-cols-[minmax(0,1fr)_65px_28px] items-end gap-2">
                <Field label={t('appliances.name')} value={a.name} maxLength={80} change={(name) => update({ ...a, name })} />
                <Field label={t('appliances.quantity')} value={String(a.quantity)} change={(v) => update({ ...a, quantity: Number(v) })} />
                <Tooltip content={t('appliances.remove')}>
                  <Button
                    size="icon-sm"
                    variant="ghost"
                    disabled={disabled || values.length <= 1}
                    aria-label={t('appliances.remove')}
                    onClick={() => change(values.filter((value) => value.id !== a.id))}
                  >
                    <Trash2 />
                  </Button>
                </Tooltip>
              </div>
              <UsageEditor usage={a.usage} label={t('usage.baseline')} change={(usage) => update({ ...a, usage })} />
              <Checkbox
                checked={a.alternative !== null}
                disabled={disabled}
                onCheckedChange={(on) => update({ ...a, alternative: on ? { ...a.usage } : null })}
              >
                {t('usage.compare')}
              </Checkbox>
              {a.alternative && (
                <div className="rounded-control border border-primary/20 bg-primary/5 p-2">
                  <UsageEditor usage={a.alternative} label={t('usage.alternative')} change={(alternative) => update({ ...a, alternative })} />
                </div>
              )}
            </article>
          )
        })
      )}
      {result && <Pages data={result} change={setOffset} />}
    </div>
  )
}
export function TariffEditor({ tariff, change }: { tariff: Tariff; change: (value: Tariff) => void }) {
  const { t } = usePlugin()
  const p = tariff.pricing
  const setPricing = (pricing: Pricing) => change({ ...tariff, pricing })
  return (
    <div className="space-y-3">
      <Field label={t('tariff.name')} value={tariff.name} maxLength={80} change={(name) => change({ ...tariff, name })} />
      <Select
        size="sm"
        value={p.kind}
        aria-label={t('tariff.kind')}
        options={(['flat', 'tiered', 'tou'] as const).map((value) => ({ value, label: t(`tariff.${value}`) }))}
        onValueChange={(kind) =>
          setPricing(
            kind === 'flat'
              ? { kind, rate: '0.6' }
              : kind === 'tou'
                ? { kind, peak: '0.9', shoulder: '0.6', valley: '0.3' }
                : {
                    kind,
                    tiers: [
                      { upTo: '180', rate: '0.55' },
                      { upTo: '400', rate: '0.65' },
                      { upTo: null, rate: '0.85' },
                    ],
                  },
          )
        }
      />
      {p.kind === 'flat' && <Field label={t('tariff.rate')} value={p.rate} change={(rate) => setPricing({ ...p, rate })} />}
      {p.kind === 'tou' && (
        <div className="grid grid-cols-3 gap-2">
          {(['peak', 'shoulder', 'valley'] as const).map((period) => (
            <Field key={period} label={t(`tariff.${period}`)} value={p[period]} change={(value) => setPricing({ ...p, [period]: value })} />
          ))}
        </div>
      )}
      {p.kind === 'tiered' && (
        <div className="space-y-2">
          {p.tiers.map((tier, i) => (
            <div key={i} className="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_28px] items-end gap-2">
              {tier.upTo === null ? (
                <p className="pb-2 text-xs text-fg-muted">{t('tariff.unlimited')}</p>
              ) : (
                <Field
                  label={t('tariff.upTo', { n: i + 1 })}
                  value={tier.upTo}
                  change={(upTo) => setPricing({ ...p, tiers: p.tiers.map((old, j) => (j === i ? { ...old, upTo } : old)) })}
                />
              )}
              <Field
                label={t('tariff.rate')}
                value={tier.rate}
                change={(rate) => setPricing({ ...p, tiers: p.tiers.map((old, j) => (j === i ? { ...old, rate } : old)) })}
              />
              {tier.upTo !== null && (
                <Tooltip content={t('tariff.removeTier')}>
                  <Button
                    size="icon-sm"
                    variant="ghost"
                    aria-label={t('tariff.removeTier')}
                    onClick={() => setPricing({ ...p, tiers: p.tiers.filter((_, j) => j !== i) })}
                  >
                    <Trash2 />
                  </Button>
                </Tooltip>
              )}
            </div>
          ))}
          <Button
            size="sm"
            disabled={p.tiers.length >= 10}
            onClick={() => setPricing({ ...p, tiers: [...p.tiers.slice(0, -1), { upTo: '500', rate: '0.75' }, p.tiers[p.tiers.length - 1]] })}
          >
            <Plus />
            {t('tariff.addTier')}
          </Button>
        </div>
      )}
      <p className="text-[11px] leading-relaxed text-fg-subtle">{t(`tariff.hint.${p.kind}`)}</p>
    </div>
  )
}
