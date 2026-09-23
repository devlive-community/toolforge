import { cloneElement, isValidElement, useRef, useState, type ReactElement, type ReactNode } from 'react'
import {
  autoUpdate,
  flip,
  FloatingFocusManager,
  FloatingPortal,
  offset,
  shift,
  useClick,
  useDismiss,
  useFloating,
  useInteractions,
  useListNavigation,
  useRole,
  type Placement,
} from '@floating-ui/react'
import { cn } from '../utils'

export interface MenuItem {
  key: string
  label: string
  icon?: ReactNode
  hint?: ReactNode
  danger?: boolean
  disabled?: boolean
  onSelect: () => void
}

export interface DropdownMenuProps {
  trigger: ReactElement<Record<string, unknown>>
  items: MenuItem[]
  placement?: Placement
  className?: string
}

export function DropdownMenu({ trigger, items, placement = 'bottom-end', className }: DropdownMenuProps) {
  const [open, setOpen] = useState(false)
  const [activeIndex, setActiveIndex] = useState<number | null>(null)
  const listRef = useRef<(HTMLElement | null)[]>([])

  const { refs, floatingStyles, context } = useFloating({
    open,
    onOpenChange: setOpen,
    placement,
    whileElementsMounted: autoUpdate,
    middleware: [offset(4), flip({ padding: 8 }), shift({ padding: 8 })],
  })
  const { getReferenceProps, getFloatingProps, getItemProps } = useInteractions([
    useClick(context),
    useDismiss(context),
    useRole(context, { role: 'menu' }),
    useListNavigation(context, { listRef, activeIndex, onNavigate: setActiveIndex, loop: true }),
  ])

  const select = (item: MenuItem) => {
    if (item.disabled) return
    setOpen(false)
    item.onSelect()
  }

  return (
    <>
      {isValidElement(trigger) &&
        cloneElement(trigger, { ref: refs.setReference, ...getReferenceProps(trigger.props) })}
      {open && (
        <FloatingPortal>
          <FloatingFocusManager context={context} modal={false} initialFocus={-1}>
            <div
              ref={refs.setFloating}
              style={floatingStyles}
              {...getFloatingProps()}
              className={cn('z-dropdown min-w-40 rounded-popover bg-elevated p-1 shadow-popover outline-none', className)}
            >
              {items.map((item, index) => (
                <button
                  key={item.key}
                  type="button"
                  role="menuitem"
                  disabled={item.disabled}
                  ref={(node) => {
                    listRef.current[index] = node
                  }}
                  tabIndex={index === activeIndex ? 0 : -1}
                  {...getItemProps({ onClick: () => select(item) })}
                  className={cn(
                    'flex w-full items-center gap-2 rounded-[7px] px-2 py-1.5 text-left text-[13px] outline-none [&_svg]:size-4',
                    item.danger ? 'text-danger' : 'text-fg',
                    index === activeIndex && 'bg-hover',
                    item.disabled && 'pointer-events-none opacity-50',
                  )}
                >
                  {item.icon && <span className="flex text-fg-muted">{item.icon}</span>}
                  <span className="flex-1 truncate">{item.label}</span>
                  {item.hint && <span className="text-xs text-fg-subtle">{item.hint}</span>}
                </button>
              ))}
            </div>
          </FloatingFocusManager>
        </FloatingPortal>
      )}
    </>
  )
}
