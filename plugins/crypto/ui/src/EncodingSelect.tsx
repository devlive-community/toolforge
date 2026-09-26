import { Select } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import type { Encoding } from './types'

export function EncodingSelect({ value, onChange, label }: { value: Encoding; onChange: (value: Encoding) => void; label: string }) {
  const { t } = usePlugin()
  return (
    <Select<Encoding>
      size="sm"
      value={value}
      onValueChange={onChange}
      className="w-28"
      aria-label={label}
      options={[
        { value: 'utf8', label: t('encodings.utf8') },
        { value: 'hex', label: 'Hex' },
        { value: 'base64', label: 'Base64' },
      ]}
    />
  )
}
