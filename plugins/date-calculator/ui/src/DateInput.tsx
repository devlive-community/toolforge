import { Button, Input } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { CalendarCheck } from 'lucide-react'

/** 日期输入框，带“今天”按钮；格式由 Rust 解析校验 */
export function DateInput({ label, value, onChange, today }: { label: string; value: string; onChange: (value: string) => void; today: string }) {
  const { t } = usePlugin()
  return (
    <label className="block space-y-1.5">
      <span className="block text-xs font-medium text-fg-muted">{label}</span>
      <span className="flex gap-2">
        <Input value={value} onChange={(e) => onChange(e.target.value)} placeholder={t('input.placeholder')} className="font-mono" aria-label={label} />
        <Button variant="ghost" onClick={() => onChange(today)}>
          <CalendarCheck />
          {t('input.today')}
        </Button>
      </span>
    </label>
  )
}
