import type { ReactNode } from 'react'
import { Check, Minus } from 'lucide-react'
import { cn } from '../utils'

export interface CheckboxProps {
  checked: boolean
  onCheckedChange: (checked: boolean) => void
  /** 半选状态（部分子项被选中） */
  indeterminate?: boolean
  disabled?: boolean
  children?: ReactNode
  className?: string
  'aria-label'?: string
}

export function Checkbox({ checked, onCheckedChange, indeterminate, disabled, children, className, ...aria }: CheckboxProps) {
  const on = checked || indeterminate
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={indeterminate ? 'mixed' : checked}
      disabled={disabled}
      onClick={() => onCheckedChange(!checked)}
      className={cn(
        'group inline-flex items-center gap-2 rounded-[5px] text-[13px] text-fg outline-none',
        'focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50',
        className,
      )}
      {...aria}
    >
      <span
        className={cn(
          'flex size-4 shrink-0 items-center justify-center rounded-[4px] border transition-colors duration-150',
          on ? 'border-primary bg-primary text-fg-on-primary' : 'border-border-strong bg-surface group-hover:border-primary',
        )}
      >
        {indeterminate ? <Minus className="size-3" strokeWidth={3} /> : checked && <Check className="size-3" strokeWidth={3} />}
      </span>
      {children}
    </button>
  )
}
