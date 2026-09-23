import type { HTMLAttributes } from 'react'
import { cn } from '../utils'

export function Kbd({ className, ...props }: HTMLAttributes<HTMLElement>) {
  return (
    <kbd
      className={cn(
        'inline-flex h-5 min-w-5 items-center justify-center rounded-[5px] border border-border bg-surface-2 px-1 font-sans text-[11px] text-fg-muted',
        className,
      )}
      {...props}
    />
  )
}
