import { useState } from 'react'
import { Button, Checkbox, Input, Modal, Switch } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { Eye, EyeOff, KeyRound, Plus, X } from 'lucide-react'
import type { MetadataEntry } from './types'

/** 编辑调用时附带的 metadata；标记为凭据的值只在当前会话保留 */
export function MetadataModal({
  open,
  onClose,
  entries,
  onChange,
}: {
  open: boolean
  onClose: () => void
  entries: MetadataEntry[]
  onChange: (entries: MetadataEntry[]) => void
}) {
  const { t } = usePlugin()
  const [revealed, setRevealed] = useState<number | null>(null)
  const edit = (index: number, patch: Partial<MetadataEntry>) =>
    onChange(entries.map((entry, i) => (i === index ? { ...entry, ...patch } : entry)))
  return (
    <Modal open={open} onOpenChange={(value) => !value && onClose()} title={t('metadata.title')} description={t('metadata.hint')} size="lg" closeLabel={t('common:close')}>
      <div className="space-y-3 px-5 pb-5">
        <p className="flex items-start gap-2 rounded-control bg-surface-2 p-3 text-xs text-fg-muted">
          <KeyRound className="size-4 shrink-0 text-primary" />
          {t('metadata.secretHint')}
        </p>
        {entries.map((entry, index) => (
          <div key={index} className="flex flex-wrap items-center gap-2">
            <Checkbox checked={entry.enabled} onCheckedChange={(enabled) => edit(index, { enabled })} aria-label={t('metadata.enabled')} />
            <Input
              size="sm"
              value={entry.key}
              onChange={(e) => edit(index, { key: e.target.value })}
              placeholder={t('metadata.key')}
              className="font-mono"
              wrapperClassName="w-44"
              aria-label={t('metadata.key')}
            />
            <Input
              size="sm"
              type={entry.secret && revealed !== index ? 'password' : 'text'}
              value={entry.value}
              onChange={(e) => edit(index, { value: e.target.value })}
              placeholder={entry.secret ? t('metadata.sessionValue') : t('metadata.value')}
              className="font-mono"
              wrapperClassName="min-w-32 flex-1"
              autoComplete="off"
              aria-label={t('metadata.value')}
            />
            {entry.secret && (
              <Button variant="ghost" size="icon-sm" onClick={() => setRevealed(revealed === index ? null : index)} aria-label={revealed === index ? t('metadata.hide') : t('metadata.reveal')}>
                {revealed === index ? <EyeOff /> : <Eye />}
              </Button>
            )}
            <label className="flex items-center gap-1.5 text-xs text-fg-muted">
              <Switch checked={entry.secret} onCheckedChange={(secret) => edit(index, { secret })} aria-label={t('metadata.secret')} />
              {t('metadata.secret')}
            </label>
            <Button variant="ghost" size="icon-sm" onClick={() => onChange(entries.filter((_, i) => i !== index))} aria-label={t('metadata.remove')}>
              <X />
            </Button>
          </div>
        ))}
        <Button size="sm" variant="outline" onClick={() => onChange([...entries, { key: '', value: '', enabled: true, secret: false }])}>
          <Plus />
          {t('metadata.add')}
        </Button>
      </div>
    </Modal>
  )
}
