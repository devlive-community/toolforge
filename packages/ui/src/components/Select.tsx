import { useRef, useState, type ReactNode } from 'react'
import {
  autoUpdate,
  flip,
  FloatingFocusManager,
  FloatingPortal,
  offset,
  shift,
  size as sizeMiddleware,
  useClick,
  useDismiss,
  useFloating,
  useInteractions,
  useListNavigation,
  useRole,
  useTypeahead,
} from '@floating-ui/react'
import { Check, ChevronDown } from 'lucide-react'
import { cn } from '../utils'

export interface SelectOption<T extends string | number> {
  label: string
  value: T
  disabled?: boolean
  icon?: ReactNode
}

export interface SelectProps<T extends string | number> {
  value: T | null
  onValueChange: (value: T) => void
  options: SelectOption<T>[]
  placeholder?: string
  disabled?: boolean
  size?: 'sm' | 'md'
  variant?: 'outline' | 'ghost'
  className?: string
  'aria-label'?: string
}

export function Select<T extends string | number>({
  value,
  onValueChange,
  options,
  placeholder,
  disabled,
  size = 'md',
  variant = 'outline',
  className,
  ...aria
}: SelectProps<T>) {
  const [open, setOpen] = useState(false)
  const [activeIndex, setActiveIndex] = useState<number | null>(null)
  const listRef = useRef<(HTMLElement | null)[]>([])
  const labelsRef = useRef(options.map((o) => o.label))
  labelsRef.current = options.map((o) => o.label)
  const selectedIndex = options.findIndex((o) => o.value === value)
  const selected = options[selectedIndex]

  const { refs, floatingStyles, context } = useFloating({
    open,
    onOpenChange: setOpen,
    placement: 'bottom-start',
    whileElementsMounted: autoUpdate,
    middleware: [
      offset(4),
      flip({ padding: 8 }),
      shift({ padding: 8 }),
      sizeMiddleware({
        apply({ rects, elements, availableHeight }) {
          elements.floating.style.minWidth = `${rects.reference.width}px`
          elements.floating.style.maxHeight = `${Math.min(280, availableHeight - 8)}px`
        },
      }),
    ],
  })

  const { getReferenceProps, getFloatingProps, getItemProps } = useInteractions([
    useClick(context),
    useDismiss(context),
    useRole(context, { role: 'listbox' }),
    useListNavigation(context, {
      listRef,
      activeIndex,
      selectedIndex: selectedIndex >= 0 ? selectedIndex : null,
      onNavigate: setActiveIndex,
      loop: true,
    }),
    useTypeahead(context, {
      listRef: labelsRef,
      activeIndex,
      selectedIndex: selectedIndex >= 0 ? selectedIndex : null,
      onMatch: open ? setActiveIndex : undefined,
    }),
  ])

  const choose = (index: number) => {
    const option = options[index]
    if (!option || option.disabled) return
    onValueChange(option.value)
    setOpen(false)
  }

  return (
    <>
      <button
        ref={refs.setReference}
        type="button"
        disabled={disabled}
        {...aria}
        {...getReferenceProps()}
        className={cn(
          'inline-flex items-center justify-between gap-1.5 rounded-control outline-none transition-colors',
          'focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50',
          size === 'sm' ? 'h-control-sm px-2 text-xs' : 'h-control-md px-2.5 text-[13px]',
          variant === 'outline'
            ? 'border border-border bg-surface hover:border-border-strong'
            : 'bg-transparent hover:bg-hover',
          open && variant === 'outline' && 'border-primary ring-2 ring-ring',
          className,
        )}
      >
        <span className={cn('flex items-center gap-1.5 truncate', selected ? 'text-fg' : 'text-fg-subtle')}>
          {selected?.icon}
          {selected?.label ?? placeholder}
        </span>
        <ChevronDown
          className={cn('size-3.5 shrink-0 text-fg-muted transition-transform duration-150', open && 'rotate-180')}
        />
      </button>
      {open && (
        <FloatingPortal>
          <FloatingFocusManager context={context} modal={false}>
            <div
              ref={refs.setFloating}
              style={floatingStyles}
              {...getFloatingProps()}
              className="z-dropdown overflow-auto rounded-popover bg-elevated p-1 shadow-popover outline-none"
            >
              {options.map((option, index) => (
                <div
                  key={String(option.value)}
                  ref={(node) => {
                    listRef.current[index] = node
                  }}
                  role="option"
                  aria-selected={index === selectedIndex}
                  aria-disabled={option.disabled}
                  tabIndex={index === activeIndex ? 0 : -1}
                  {...getItemProps({
                    onClick: () => choose(index),
                    onKeyDown: (event) => {
                      if (event.key === 'Enter' || event.key === ' ') {
                        event.preventDefault()
                        choose(index)
                      }
                    },
                  })}
                  className={cn(
                    'flex cursor-pointer items-center gap-2 rounded-[7px] py-1.5 pr-2 pl-2 text-[13px] text-fg outline-none',
                    index === activeIndex && 'bg-hover',
                    option.disabled && 'pointer-events-none opacity-50',
                  )}
                >
                  {option.icon}
                  <span className="flex-1 truncate">{option.label}</span>
                  <Check className={cn('size-3.5 text-primary', index !== selectedIndex && 'invisible')} />
                </div>
              ))}
            </div>
          </FloatingFocusManager>
        </FloatingPortal>
      )}
    </>
  )
}
