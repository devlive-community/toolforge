import { useRef } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { cn } from '@toolforge/ui'
import type { Line, Row, Tag } from './types'

const ROW_HEIGHT = 22

const sideStyle: Record<Tag, { left: string; right: string }> = {
  equal: { left: '', right: '' },
  delete: { left: 'bg-danger-soft/70', right: 'bg-surface-2' },
  insert: { left: 'bg-surface-2', right: 'bg-success-soft/70' },
  change: { left: 'bg-danger-soft/50', right: 'bg-success-soft/50' },
}

function Cell({ line, className, emphasis }: { line: Line | null; className: string; emphasis: string }) {
  return (
    <div className={cn('flex min-w-0 items-center', className)}>
      <span className="w-12 shrink-0 pr-2 text-right text-fg-subtle tabular-nums select-none">{line?.number ?? ''}</span>
      <span className="min-w-0 flex-1 overflow-hidden whitespace-pre text-ellipsis" data-selectable>
        {line?.segments.map((segment, index) => (
          <span key={index} className={segment.emphasized ? cn('rounded-[3px]', emphasis) : undefined}>
            {segment.text}
          </span>
        ))}
      </span>
    </div>
  )
}

/** 并排对比视图：虚拟滚动，大文件也能流畅浏览 */
export function SideBySide({ rows }: { rows: Row[] }) {
  const parent = useRef<HTMLDivElement>(null)
  // TanStack Virtual 返回的函数无法被 React Compiler 记忆化
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parent.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 30,
  })
  return (
    <div ref={parent} className="h-full overflow-auto font-mono text-[12.5px]">
      <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
        {virtualizer.getVirtualItems().map((item) => {
          const row = rows[item.index]
          const style = sideStyle[row.tag]
          return (
            <div
              key={item.index}
              className="absolute left-0 grid w-full grid-cols-2 divide-x divide-border"
              style={{ height: ROW_HEIGHT, transform: `translateY(${item.start}px)` }}
            >
              <Cell line={row.left} className={style.left} emphasis="bg-danger/25" />
              <Cell line={row.right} className={style.right} emphasis="bg-success/30" />
            </div>
          )
        })}
      </div>
    </div>
  )
}
