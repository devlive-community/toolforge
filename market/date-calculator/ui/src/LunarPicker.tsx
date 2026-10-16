import { NumberInput, Select, Switch } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import type { LunarDate } from './types'

const MONTHS = Array.from({ length: 12 }, (_, i) => i + 1)
const DAYS = Array.from({ length: 30 }, (_, i) => i + 1)

/** 农历日期选择：年（可省略）、月、日、是否闰月；合法性由 Rust 校验 */
export function LunarPicker({ value, onChange, withYear = true }: { value: LunarDate; onChange: (value: LunarDate) => void; withYear?: boolean }) {
  const { t } = usePlugin()
  const set = (patch: Partial<LunarDate>) => onChange({ ...value, ...patch })
  return (
    <div className="flex flex-wrap items-center gap-2">
      {withYear && (
        <NumberInput value={value.year} onValueChange={(year) => set({ year })} min={1900} max={2100} className="w-32" aria-label={t('lunar.year')} />
      )}
      <Select<string>
        value={String(value.month)}
        onValueChange={(month) => set({ month: Number(month) })}
        className="w-28"
        aria-label={t('lunar.month')}
        options={MONTHS.map((m) => ({ value: String(m), label: t(`lunar.months.${m}`) }))}
      />
      <Select<string>
        value={String(value.day)}
        onValueChange={(day) => set({ day: Number(day) })}
        className="w-24"
        aria-label={t('lunar.day')}
        options={DAYS.map((d) => ({ value: String(d), label: t(`lunar.days.${d}`) }))}
      />
      <label className="flex items-center gap-2 text-[13px] text-fg">
        <Switch size="sm" checked={value.leap} onCheckedChange={(leap) => set({ leap })} aria-label={t('lunar.leap')} />
        {t('lunar.leap')}
      </label>
    </div>
  )
}
