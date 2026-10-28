import { useEffect, useRef, useState } from 'react'
import { cn } from '@toolforge/ui'
import { dropIndex } from './history'
import { frameSrc } from './store'

/** 按下后移动超过这个距离才算拖动，否则是点选 */
const DRAG_THRESHOLD = 5

interface Target {
  index: number
  side: 'before' | 'after'
}

interface TimelineProps {
  frames: string[]
  contents: Record<string, string>
  width: number
  height: number
  current: number
  disabled: boolean
  onSelect: (index: number) => void
  onMove: (from: number, to: number) => void
  frameLabel: (index: number) => string
}

export function Timeline({ frames, contents, width, height, current, disabled, onSelect, onMove, frameLabel }: TimelineProps) {
  const pressed = useRef<{ index: number; x: number; y: number } | null>(null)
  const [dragging, setDragging] = useState<number | null>(null)
  const [target, setTarget] = useState<Target | null>(null)
  // 窗口监听只注册一次，通过 ref 读取最新回调
  const moveRef = useRef(onMove)
  const selectRef = useRef(onSelect)
  useEffect(() => {
    moveRef.current = onMove
    selectRef.current = onSelect
  })

  useEffect(() => {
    let active: number | null = null
    let last: Target | null = null
    const locate = (x: number, y: number): Target | null => {
      const card = (document.elementFromPoint(x, y) as HTMLElement | null)?.closest<HTMLElement>('[data-frame-index]')
      if (!card) return null
      const index = Number(card.dataset.frameIndex)
      const rect = card.getBoundingClientRect()
      return { index, side: x < rect.left + rect.width / 2 ? 'before' : 'after' }
    }
    const onPointerMove = (event: PointerEvent) => {
      const start = pressed.current
      if (!start) return
      if (active === null) {
        if (Math.hypot(event.clientX - start.x, event.clientY - start.y) < DRAG_THRESHOLD) return
        active = start.index
        setDragging(active)
      }
      last = locate(event.clientX, event.clientY)
      setTarget(last)
    }
    const finish = (commit: boolean) => {
      const start = pressed.current
      pressed.current = null
      if (active !== null && commit && last) {
        const to = dropIndex(active, last.index, last.side)
        if (to !== active) moveRef.current(active, to)
      } else if (active === null && commit && start) {
        // 没有拖动：当作点选（在 pointerup 里处理，避免拖动后又触发点击）
        selectRef.current(start.index)
      }
      active = null
      last = null
      setDragging(null)
      setTarget(null)
    }
    const onUp = () => finish(true)
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && active !== null) finish(false)
    }
    window.addEventListener('pointermove', onPointerMove)
    window.addEventListener('pointerup', onUp)
    window.addEventListener('keydown', onKey)
    return () => {
      window.removeEventListener('pointermove', onPointerMove)
      window.removeEventListener('pointerup', onUp)
      window.removeEventListener('keydown', onKey)
    }
  }, [])

  return (
    <div className="flex gap-2 overflow-x-auto pb-1" role="listbox" aria-orientation="horizontal">
      {frames.map((id, index) => {
        const src = frameSrc(contents[id])
        const marker = target && target.index === index && dragging !== null ? target.side : null
        return (
          <div
            key={id}
            data-frame-index={index}
            role="option"
            aria-selected={index === current}
            aria-label={frameLabel(index)}
            tabIndex={0}
            onPointerDown={(event) => {
              if (disabled || event.button !== 0) return
              pressed.current = { index, x: event.clientX, y: event.clientY }
            }}
            onKeyDown={(event) => {
              if (event.key === 'Enter' || event.key === ' ') onSelect(index)
            }}
            className={cn(
              'relative shrink-0 cursor-pointer select-none rounded-control border-2 bg-surface p-0.5 outline-none focus-visible:ring-2 focus-visible:ring-ring',
              index === current ? 'border-primary' : 'border-border',
              dragging === index && 'opacity-40',
              disabled && 'cursor-not-allowed',
            )}
          >
            {marker && (
              <span
                className={cn('absolute inset-y-0 w-0.5 bg-primary', marker === 'before' ? '-left-1.5' : '-right-1.5')}
                aria-hidden
              />
            )}
            <div className="w-20 overflow-hidden rounded-[4px] bg-bg" style={{ aspectRatio: `${width} / ${height}` }}>
              {src && <img src={src} alt="" draggable={false} className="h-full w-full object-contain" />}
            </div>
            <span className="absolute bottom-1 left-1 rounded bg-elevated px-1 text-[10px] text-fg-muted">{index + 1}</span>
          </div>
        )
      })}
    </div>
  )
}
