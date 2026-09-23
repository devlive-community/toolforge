import { cn } from '../utils'

export interface ProgressProps {
  /** 0–100；不传或为 null 时显示不确定进度 */
  value?: number | null
  size?: 'sm' | 'md'
  tone?: 'primary' | 'success' | 'danger'
  className?: string
  'aria-label'?: string
}

const tones = { primary: 'bg-primary', success: 'bg-success', danger: 'bg-danger' }

export function Progress({ value, size = 'md', tone = 'primary', className, ...aria }: ProgressProps) {
  const indeterminate = value === undefined || value === null
  const clamped = indeterminate ? 0 : Math.min(100, Math.max(0, value))
  return (
    <div
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={indeterminate ? undefined : Math.round(clamped)}
      {...aria}
      className={cn('relative w-full overflow-hidden rounded-full bg-active', size === 'sm' ? 'h-1' : 'h-1.5', className)}
    >
      <div
        className={cn(
          'h-full rounded-full transition-[width] duration-200 ease-out',
          tones[tone],
          indeterminate && 'absolute w-1/3 animate-indeterminate',
        )}
        style={indeterminate ? undefined : { width: `${clamped}%` }}
      />
    </div>
  )
}
