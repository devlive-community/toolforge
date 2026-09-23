import { useState, type ReactNode } from 'react'
import { Button, Tooltip, cn, toast, type ButtonProps } from '@toolforge/ui'
import { Check, Copy } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { host } from './host'
import { useErrorMessage } from './i18n'

/** 复制文本到剪贴板，成功后短暂显示 ✓ 并提示 */
export function useCopy() {
  const { t } = useTranslation('common')
  const errorMessage = useErrorMessage()
  const [copied, setCopied] = useState(false)
  const copy = async (text: string) => {
    try {
      await host.clipboard.writeText(text)
      setCopied(true)
      toast.success(t('copied'))
      setTimeout(() => setCopied(false), 1200)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  return { copy, copied }
}

export interface CopyButtonProps extends Omit<ButtonProps, 'onClick' | 'children'> {
  text: string
  /** 显示文字；不传则为仅图标按钮 */
  label?: ReactNode
}

export function CopyButton({ text, label, size, variant = 'ghost', className, ...props }: CopyButtonProps) {
  const { t } = useTranslation('common')
  const { copy, copied } = useCopy()
  const icon = copied ? <Check className="text-success" /> : <Copy />
  if (label) {
    return (
      <Button size={size ?? 'sm'} variant={variant} className={className} onClick={() => copy(text)} {...props}>
        {icon}
        {label}
      </Button>
    )
  }
  return (
    <Tooltip content={t('copy')}>
      <Button size={size ?? 'icon-sm'} variant={variant} aria-label={t('copy')} className={className} onClick={() => copy(text)} {...props}>
        {icon}
      </Button>
    </Tooltip>
  )
}

/** 结果行：标签 + 等宽值 + 悬停显示的复制按钮 */
export function ValueRow({ label, value, labelWidth = 'w-20' }: { label: ReactNode; value: string; labelWidth?: string }) {
  return (
    <div className="group flex items-center gap-3 rounded-control px-3 py-2 hover:bg-hover">
      <span className={cn('shrink-0 text-xs font-semibold text-fg-muted', labelWidth)}>{label}</span>
      <code className="min-w-0 flex-1 font-mono text-[12.5px] break-all text-fg" data-selectable>
        {value}
      </code>
      <CopyButton text={value} className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100" />
    </div>
  )
}
