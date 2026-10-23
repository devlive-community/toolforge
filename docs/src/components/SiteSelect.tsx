'use client'
import { useEffect, useId, useRef, useState } from 'react'
import { Check, ChevronDown } from 'lucide-react'

export interface SiteSelectOption<T extends string> {
  value: T
  label: string
}

/**
 * 自绘下拉选择：替代系统原生 select，外观与站点一致。
 * 键盘：Enter / Space / ↓ 打开，↑ ↓ 移动，Enter 选择，Esc 关闭；点击外部关闭
 */
export function SiteSelect<T extends string>({
  value,
  options,
  onChange,
  label,
  className,
}: {
  value: T
  options: SiteSelectOption<T>[]
  onChange: (value: T) => void
  label: string
  className?: string
}) {
  const [open, setOpen] = useState(false)
  const [active, setActive] = useState(0)
  const root = useRef<HTMLDivElement>(null)
  const button = useRef<HTMLButtonElement>(null)
  const id = useId()
  const selected = options.find((option) => option.value === value) ?? options[0]

  useEffect(() => {
    if (!open) return
    const close = (event: MouseEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false)
    }
    document.addEventListener('mousedown', close)
    return () => document.removeEventListener('mousedown', close)
  }, [open])

  const show = () => {
    setActive(Math.max(0, options.findIndex((option) => option.value === value)))
    setOpen(true)
  }
  const choose = (index: number) => {
    onChange(options[index].value)
    setOpen(false)
    button.current?.focus()
  }

  return (
    <div className={`site-dropdown${className ? ` ${className}` : ''}`} ref={root}>
      <button
        ref={button}
        type="button"
        className="site-dropdown-trigger"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={`${id}-list`}
        aria-label={label}
        onClick={() => (open ? setOpen(false) : show())}
        onKeyDown={(event) => {
          if (!open && ['ArrowDown', 'Enter', ' '].includes(event.key)) {
            event.preventDefault()
            show()
            return
          }
          if (!open) return
          if (event.key === 'Escape') {
            event.preventDefault()
            setOpen(false)
          } else if (event.key === 'ArrowDown') {
            event.preventDefault()
            setActive((index) => Math.min(options.length - 1, index + 1))
          } else if (event.key === 'ArrowUp') {
            event.preventDefault()
            setActive((index) => Math.max(0, index - 1))
          } else if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault()
            choose(active)
          } else if (event.key === 'Tab') {
            setOpen(false)
          }
        }}
      >
        <span>{selected?.label}</span>
        <ChevronDown aria-hidden="true" />
      </button>
      {open && (
        <ul className="site-dropdown-list" role="listbox" id={`${id}-list`} aria-label={label}>
          {options.map((option, index) => (
            <li
              key={option.value}
              role="option"
              aria-selected={option.value === value}
              data-active={index === active}
              onMouseEnter={() => setActive(index)}
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => choose(index)}
            >
              <span>{option.label}</span>
              {option.value === value && <Check aria-hidden="true" />}
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
