import { useRef, type KeyboardEvent, type ReactNode } from 'react'
import { cn } from '../utils'

export interface TabItem<T extends string> {
  value: T
  label: ReactNode
  icon?: ReactNode
  disabled?: boolean
}

export interface TabsProps<T extends string> {
  value: T
  onValueChange: (value: T) => void
  items: TabItem<T>[]
  /** solid：带边框容器 + 选中实心主色；underline：选中浅底 + 底部主色线 */
  variant?: 'solid' | 'underline'
  className?: string
  'aria-label'?: string
}

export function Tabs<T extends string>({
  value,
  onValueChange,
  items,
  variant = 'solid',
  className,
  ...aria
}: TabsProps<T>) {
  const refs = useRef<(HTMLButtonElement | null)[]>([])

  const onKeyDown = (event: KeyboardEvent, index: number) => {
    if (event.key !== 'ArrowRight' && event.key !== 'ArrowLeft') return
    event.preventDefault()
    const step = event.key === 'ArrowRight' ? 1 : -1
    for (let i = 1; i <= items.length; i++) {
      const next = (index + step * i + items.length) % items.length
      if (!items[next].disabled) {
        refs.current[next]?.focus()
        onValueChange(items[next].value)
        return
      }
    }
  }

  return (
    <div
      role="tablist"
      {...aria}
      className={cn(
        'flex items-center',
        variant === 'solid' && 'gap-1 rounded-card border border-border bg-surface p-1 shadow-card',
        variant === 'underline' && 'gap-1',
        className,
      )}
    >
      {items.map((item, index) => {
        const selected = item.value === value
        return (
          <button
            key={item.value}
            ref={(node) => {
              refs.current[index] = node
            }}
            type="button"
            role="tab"
            aria-selected={selected}
            tabIndex={selected ? 0 : -1}
            disabled={item.disabled}
            onClick={() => onValueChange(item.value)}
            onKeyDown={(event) => onKeyDown(event, index)}
            className={cn(
              'inline-flex items-center justify-center gap-2 whitespace-nowrap text-[13px] font-medium outline-none transition-colors duration-150',
              'focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 [&_svg]:size-4',
              variant === 'solid' && [
                'h-9 min-w-28 rounded-[9px] px-4',
                selected ? 'bg-primary text-fg-on-primary shadow-card' : 'text-fg-muted hover:bg-hover hover:text-fg',
              ],
              variant === 'underline' && [
                'relative h-9 flex-1 rounded-t-[8px] px-3',
                selected
                  ? 'bg-primary-soft text-primary-fg after:absolute after:inset-x-0 after:bottom-0 after:h-0.5 after:bg-primary'
                  : 'text-fg-muted hover:bg-hover hover:text-fg',
              ],
            )}
          >
            {item.icon}
            {item.label}
          </button>
        )
      })}
    </div>
  )
}
