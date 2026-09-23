import { forwardRef, type InputHTMLAttributes, type ReactNode } from 'react'
import { cn } from '../utils'

export interface InputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, 'size'> {
  leading?: ReactNode
  trailing?: ReactNode
  size?: 'sm' | 'md' | 'lg'
  invalid?: boolean
  wrapperClassName?: string
}

const heights = { sm: 'h-control-sm text-xs', md: 'h-control-md text-[13px]', lg: 'h-control-lg text-sm' }

export const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ leading, trailing, size = 'md', invalid, className, wrapperClassName, ...props }, ref) => (
    <div
      className={cn(
        'flex items-center gap-2 rounded-control border bg-surface px-2.5 transition-colors',
        'focus-within:border-primary focus-within:ring-2 focus-within:ring-ring',
        invalid ? 'border-danger' : 'border-border hover:border-border-strong',
        heights[size],
        wrapperClassName,
      )}
    >
      {leading && <span className="flex shrink-0 text-fg-subtle [&_svg]:size-4">{leading}</span>}
      <input
        ref={ref}
        className={cn('h-full min-w-0 flex-1 bg-transparent text-fg outline-none placeholder:text-fg-subtle', className)}
        aria-invalid={invalid || undefined}
        {...props}
      />
      {trailing && <span className="flex shrink-0 items-center text-fg-subtle">{trailing}</span>}
    </div>
  ),
)
Input.displayName = 'Input'
