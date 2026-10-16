import { NumberInput, Select } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { Field } from './Field'
import { PRESET_LAYOUTS, SNOWFLAKE_PRESETS, timeBits, type SnowflakeLayout, type SnowflakeOptions, type SnowflakePreset } from './types'

const maxOf = (bits: number) => 2 ** bits - 1

export function SnowflakeSettings({ value, onChange }: { value: SnowflakeOptions; onChange: (next: SnowflakeOptions) => void }) {
  const { t } = usePlugin()
  const layout = value.preset === 'custom' ? value.layout : PRESET_LAYOUTS[value.preset]
  const set = (patch: Partial<SnowflakeOptions>) => onChange({ ...value, ...patch })
  const setLayout = (patch: Partial<SnowflakeLayout>) => set({ layout: { ...value.layout, ...patch } })
  const bits = (key: 'datacenterBits' | 'workerBits' | 'sequenceBits', max: number) => (
    <Field label={t(`snowflake.${key}`)}>
      <NumberInput value={value.layout[key]} onValueChange={(n) => setLayout({ [key]: n })} min={key === 'sequenceBits' ? 1 : 0} max={max} className="w-full" aria-label={t(`snowflake.${key}`)} />
    </Field>
  )

  return (
    <>
      <Field label={t('snowflake.preset')}>
        <Select<SnowflakePreset>
          value={value.preset}
          onValueChange={(preset) => set({ preset })}
          className="w-full"
          aria-label={t('snowflake.preset')}
          options={[...SNOWFLAKE_PRESETS, 'custom' as const].map((preset) => ({ value: preset, label: t(`snowflake.presets.${preset}`) }))}
        />
      </Field>

      {value.preset === 'custom' && (
        <>
          <Field label={t('snowflake.epoch')} hint={t('snowflake.epochHint')}>
            <NumberInput value={value.layout.epochMs} onValueChange={(epochMs) => setLayout({ epochMs })} min={0} className="w-full" aria-label={t('snowflake.epoch')} />
          </Field>
          <Field label={t('snowflake.unit')}>
            <Select<string>
              value={String(value.layout.unitMs)}
              onValueChange={(unit) => setLayout({ unitMs: Number(unit) })}
              className="w-full"
              aria-label={t('snowflake.unit')}
              options={[
                { value: '1', label: t('snowflake.units.ms') },
                { value: '1000', label: t('snowflake.units.s') },
              ]}
            />
          </Field>
          <div className="grid grid-cols-3 gap-2">
            {bits('datacenterBits', 20)}
            {bits('workerBits', 30)}
            {bits('sequenceBits', 30)}
          </div>
        </>
      )}

      <div className="grid grid-cols-2 gap-2">
        {layout.datacenterBits > 0 && (
          <Field label={t('snowflake.datacenterId')}>
            <NumberInput value={Math.min(value.datacenterId, maxOf(layout.datacenterBits))} onValueChange={(datacenterId) => set({ datacenterId })} min={0} max={maxOf(layout.datacenterBits)} className="w-full" aria-label={t('snowflake.datacenterId')} />
          </Field>
        )}
        {layout.workerBits > 0 && (
          <Field label={t('snowflake.workerId')}>
            <NumberInput value={Math.min(value.workerId, maxOf(layout.workerBits))} onValueChange={(workerId) => set({ workerId })} min={0} max={maxOf(layout.workerBits)} className="w-full" aria-label={t('snowflake.workerId')} />
          </Field>
        )}
      </div>

      <p className="text-[11px] leading-relaxed text-fg-subtle">
        {t('snowflake.layout', {
          time: timeBits(layout),
          unit: t(layout.unitMs === 1000 ? 'snowflake.units.s' : 'snowflake.units.ms'),
          datacenter: layout.datacenterBits,
          worker: layout.workerBits,
          sequence: layout.sequenceBits,
        })}
        <br />
        {t('snowflake.monotonic')}
      </p>
    </>
  )
}

/** 生成时把预设之外的字段剔除，避免自定义布局残留影响预设 */
export function snowflakeArgs(value: SnowflakeOptions) {
  const layout = value.preset === 'custom' ? value.layout : PRESET_LAYOUTS[value.preset]
  return {
    preset: value.preset,
    layout: value.preset === 'custom' ? value.layout : undefined,
    datacenterId: Math.min(value.datacenterId, maxOf(layout.datacenterBits)),
    workerId: Math.min(value.workerId, maxOf(layout.workerBits)),
  }
}
