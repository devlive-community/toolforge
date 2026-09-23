import type { ReactNode } from 'react'
import {
  FloatingFocusManager,
  FloatingOverlay,
  FloatingPortal,
  useDismiss,
  useFloating,
  useInteractions,
  useRole,
} from '@floating-ui/react'
import { X } from 'lucide-react'
import { cn } from '../utils'
import { Button } from './Button'

export interface DrawerProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  title: ReactNode
  actions?: ReactNode
  children?: ReactNode
  className?: string
  closeLabel?: string
}

/** 右侧抽屉 */
export function Drawer({ open, onOpenChange, title, actions, children, className, closeLabel = 'Close' }: DrawerProps) {
  const { refs, context } = useFloating({ open, onOpenChange })
  const { getFloatingProps } = useInteractions([
    useDismiss(context, { outsidePressEvent: 'mousedown' }),
    useRole(context, { role: 'dialog' }),
  ])
  if (!open) return null
  return (
    <FloatingPortal>
      <FloatingOverlay
        lockScroll
        // floating-ui 默认 overflow: auto，滑入动画会短暂溢出而闪现滚动条
        style={{ overflow: 'hidden' }}
        className="z-modal flex animate-fade-in justify-end bg-overlay"
      >
        <FloatingFocusManager context={context}>
          <aside
            ref={refs.setFloating}
            {...getFloatingProps()}
            className={cn(
              'flex h-full w-[min(560px,92vw)] animate-slide-in-right flex-col border-l border-border bg-elevated shadow-modal outline-none',
              className,
            )}
          >
            <header className="flex h-14 shrink-0 items-center gap-2 border-b border-border px-4">
              <h2 className="text-[15px] font-semibold text-fg">{title}</h2>
              <div className="ml-auto flex items-center gap-1.5">
                {actions}
                <Button variant="ghost" size="icon-sm" aria-label={closeLabel} onClick={() => onOpenChange(false)}>
                  <X />
                </Button>
              </div>
            </header>
            <div className="min-h-0 flex-1 overflow-auto">{children}</div>
          </aside>
        </FloatingFocusManager>
      </FloatingOverlay>
    </FloatingPortal>
  )
}
