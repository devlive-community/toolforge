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

export interface ModalProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  title?: ReactNode
  description?: ReactNode
  footer?: ReactNode
  children?: ReactNode
  size?: 'sm' | 'md' | 'lg'
  /** 贴近顶部显示（命令面板等） */
  position?: 'center' | 'top'
  hideClose?: boolean
  className?: string
  closeLabel?: string
}

const widths = { sm: 'max-w-sm', md: 'max-w-lg', lg: 'max-w-2xl' }

export function Modal({
  open,
  onOpenChange,
  title,
  description,
  footer,
  children,
  size = 'md',
  position = 'center',
  hideClose,
  className,
  closeLabel = 'Close',
}: ModalProps) {
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
        className={cn(
          'z-modal flex justify-center bg-overlay px-4 backdrop-blur-[2px] animate-fade-in',
          position === 'top' ? 'items-start pt-[12vh]' : 'items-center',
        )}
      >
        <FloatingFocusManager context={context}>
          <div
            ref={refs.setFloating}
            {...getFloatingProps()}
            className={cn(
              'flex max-h-[80vh] w-full animate-pop-in flex-col overflow-hidden rounded-card bg-elevated shadow-modal outline-none',
              widths[size],
              className,
            )}
          >
            {title !== undefined && (
              <div className="flex items-start justify-between gap-4 px-5 pt-4 pb-3">
                <div className="min-w-0">
                  <h2 className="text-[15px] font-semibold text-fg">{title}</h2>
                  {description && <p className="mt-1 text-[13px] text-fg-muted">{description}</p>}
                </div>
                {!hideClose && (
                  <Button variant="ghost" size="icon-sm" aria-label={closeLabel} onClick={() => onOpenChange(false)}>
                    <X />
                  </Button>
                )}
              </div>
            )}
            <div className="min-h-0 flex-1 overflow-auto">{children}</div>
            {footer && <div className="flex justify-end gap-2 border-t border-border px-5 py-3">{footer}</div>}
          </div>
        </FloatingFocusManager>
      </FloatingOverlay>
    </FloatingPortal>
  )
}
