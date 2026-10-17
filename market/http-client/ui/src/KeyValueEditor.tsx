import { Button, Checkbox, Input, Tooltip } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { X } from 'lucide-react'
import { emptyPair, type Pair } from './types'

/** 键值对编辑器：最后一行始终为空行，输入后自动追加新行 */
export function KeyValueEditor({ pairs, onChange, keyPlaceholder }: { pairs: Pair[]; onChange: (pairs: Pair[]) => void; keyPlaceholder: string }) {
  const { t } = usePlugin()
  const rows = pairs.length === 0 || pairs[pairs.length - 1].key || pairs[pairs.length - 1].value ? [...pairs, emptyPair()] : pairs

  const update = (index: number, patch: Partial<Pair>) => {
    const next = rows.map((row, i) => (i === index ? { ...row, ...patch } : row))
    onChange(next.filter((row, i) => i < next.length - 1 || row.key || row.value))
  }

  return (
    <div className="space-y-1.5">
      {rows.map((row, index) => {
        const placeholder = index === rows.length - 1
        return (
          <div key={index} className="flex items-center gap-2">
            <Checkbox
              checked={row.enabled}
              disabled={placeholder}
              onCheckedChange={(enabled) => update(index, { enabled })}
              aria-label={t('request.enabled')}
            />
            <Input
              size="sm"
              value={row.key}
              onChange={(event) => update(index, { key: event.target.value })}
              placeholder={keyPlaceholder}
              className="font-mono"
              wrapperClassName="w-2/5"
              aria-label={keyPlaceholder}
            />
            <Input
              size="sm"
              value={row.value}
              onChange={(event) => update(index, { value: event.target.value })}
              placeholder={t('request.value')}
              className="font-mono"
              wrapperClassName="flex-1"
              aria-label={t('request.value')}
            />
            <Tooltip content={t('request.remove')}>
              <Button
                size="icon-sm"
                variant="ghost"
                aria-label={t('request.remove')}
                disabled={placeholder}
                onClick={() => onChange(pairs.filter((_, i) => i !== index))}
              >
                <X />
              </Button>
            </Tooltip>
          </div>
        )
      })}
    </div>
  )
}
