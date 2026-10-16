import { Select } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import type { StoredScheme } from './types'

/** 节假日方案选择：内置选项（可选）加上用户方案 */
export function SchemeSelect({
  schemes,
  value,
  onChange,
  builtins,
  label,
}: {
  schemes: StoredScheme[]
  value: string
  onChange: (value: string) => void
  /** 排在前面的内置选项：值与显示文案 */
  builtins: { value: string; label: string }[]
  label: string
}) {
  const { t } = usePlugin()
  const known = builtins.some((b) => b.value === value) || schemes.some((s) => s.id === value)
  return (
    <Select<string>
      size="sm"
      value={known ? value : builtins[0]?.value ?? null}
      onValueChange={onChange}
      aria-label={label}
      placeholder={t('schemes.pick')}
      options={[...builtins, ...schemes.map((scheme) => ({ value: scheme.id, label: scheme.name }))]}
    />
  )
}
