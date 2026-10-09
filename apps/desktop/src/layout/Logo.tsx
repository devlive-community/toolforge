import { cn } from '@toolforge/ui'
import logo from '../../src-tauri/icons/logo.svg'

export function Logo({ className }: { className?: string }) {
  return (
    <span className={cn('inline-flex shrink-0 items-center justify-center', className)}>
      <img src={logo} className="size-full select-none" alt="" aria-hidden draggable={false} />
    </span>
  )
}
