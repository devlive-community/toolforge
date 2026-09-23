import { useState, type KeyboardEvent } from 'react'
import { Minus, Plus } from 'lucide-react'
import { cn } from '../utils'

export interface NumberInputProps {
  value: number
  onValueChange: (value: number) => void
  min?: number
  max?: number
  step?: number
  size?: 'sm' | 'md'
  disabled?: boolean
  className?: string
  'aria-label'?: string
  decrementLabel?: string
  incrementLabel?: string
}

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value))

/** 数字输入：两侧步进按钮，↑↓ 键调整，失焦时校正到合法范围（替代原生 type="number"） */
export function NumberInput({
  value,
  onValueChange,
  min = Number.MIN_SAFE_INTEGER,
  max = Number.MAX_SAFE_INTEGER,
  step = 1,
  size = 'md',
  disabled,
  className,
  decrementLabel = 'Decrease',
  incrementLabel = 'Increase',
  ...aria
}: NumberInputProps) {
  const [draft, setDraft] = useState<string | null>(null)

  const commit = (next: number) => {
    if (Number.isFinite(next)) onValueChange(clamp(next, min, max))
    setDraft(null)
  }

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
      event.preventDefault()
      commit(value + (event.key === 'ArrowUp' ? step : -step))
    } else if (event.key === 'Enter') {
      commit(Number(draft ?? value))
    }
  }

  const button = 'flex h-full w-7 shrink-0 items-center justify-center text-fg-muted transition-colors hover:bg-hover hover:text-fg disabled:pointer-events-none disabled:opacity-40 [&_svg]:size-3.5'

  return (
    <div
      className={cn(
        'inline-flex items-center overflow-hidden rounded-control border border-border bg-surface transition-colors',
        'focus-within:border-primary focus-within:ring-2 focus-within:ring-ring hover:border-border-strong',
        size === 'sm' ? 'h-control-sm text-xs' : 'h-control-md text-[13px]',
        disabled && 'pointer-events-none opacity-50',
        className,
      )}
    >
      <button type="button" tabIndex={-1} aria-label={decrementLabel} className={button} disabled={disabled || value <= min} onClick={() => commit(value - step)}>
        <Minus />
      </button>
      <input
        type="text"
        inputMode="numeric"
        disabled={disabled}
        value={draft ?? String(value)}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={() => draft !== null && commit(Number(draft))}
        onKeyDown={onKeyDown}
        className="h-full w-full min-w-0 bg-transparent text-center text-fg tabular-nums outline-none"
        {...aria}
      />
      <button type="button" tabIndex={-1} aria-label={incrementLabel} className={button} disabled={disabled || value >= max} onClick={() => commit(value + step)}>
        <Plus />
      </button>
    </div>
  )
}
