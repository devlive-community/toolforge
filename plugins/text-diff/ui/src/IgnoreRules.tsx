import { Button, Checkbox, DropdownMenu, Input, Tooltip } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { ChevronDown, Plus, X } from 'lucide-react'

export interface IgnoreRule {
  pattern: string
  enabled: boolean
}

/** 常用的噪音规则 */
const COMMON: { key: string; pattern: string }[] = [
  { key: 'datetime', pattern: String.raw`\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:[.,]\d+)?(?:Z|[+-]\d{2}:?\d{2})?` },
  { key: 'time', pattern: String.raw`\b\d{2}:\d{2}:\d{2}(?:[.,]\d+)?\b` },
  { key: 'uuid', pattern: String.raw`\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b` },
  { key: 'hex', pattern: String.raw`\b(?:0x)?[0-9a-fA-F]{8,}\b` },
  { key: 'number', pattern: String.raw`\d+` },
  { key: 'bracketId', pattern: String.raw`\[\d+\]` },
]

export function IgnoreRules({ rules, onChange, invalidIndex }: { rules: IgnoreRule[]; onChange: (rules: IgnoreRule[]) => void; invalidIndex: number | null }) {
  const { t } = usePlugin()
  const update = (index: number, patch: Partial<IgnoreRule>) => onChange(rules.map((rule, i) => (i === index ? { ...rule, ...patch } : rule)))
  const add = (pattern: string) => onChange([...rules, { pattern, enabled: true }])

  return (
    <section className="shrink-0 space-y-2 rounded-card border border-border bg-surface px-3 py-2.5 shadow-card">
      <div className="flex flex-wrap items-center gap-2">
        <p className="text-xs font-medium text-fg-muted">{t('ignore.title')}</p>
        <p className="text-[11px] text-fg-subtle">{t('ignore.hint')}</p>
        <div className="ml-auto flex items-center gap-1.5">
          <DropdownMenu
            trigger={
              <Button size="sm">
                {t('ignore.common')}
                <ChevronDown />
              </Button>
            }
            items={COMMON.map((item) => ({ key: item.key, label: t(`ignore.presets.${item.key}`), onSelect: () => add(item.pattern) }))}
          />
          <Button size="sm" onClick={() => add('')}>
            <Plus />
            {t('ignore.add')}
          </Button>
        </div>
      </div>
      {rules.length === 0 ? (
        <p className="text-xs text-fg-subtle">{t('ignore.empty')}</p>
      ) : (
        <ul className="space-y-1.5">
          {rules.map((rule, index) => (
            <li key={index} className="flex items-center gap-2">
              <Checkbox checked={rule.enabled} onCheckedChange={(enabled) => update(index, { enabled })} aria-label={t('ignore.enabled')} />
              <Input
                size="sm"
                value={rule.pattern}
                onChange={(event) => update(index, { pattern: event.target.value })}
                placeholder={t('ignore.placeholder')}
                className="font-mono text-xs"
                wrapperClassName="min-w-0 flex-1"
                invalid={invalidIndex === index}
                aria-label={t('ignore.pattern', { index: index + 1 })}
              />
              <Tooltip content={t('ignore.remove')}>
                <Button size="icon-sm" variant="ghost" aria-label={t('ignore.remove')} onClick={() => onChange(rules.filter((_, i) => i !== index))}>
                  <X />
                </Button>
              </Tooltip>
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}
