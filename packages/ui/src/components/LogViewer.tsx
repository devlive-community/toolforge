import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { ArrowDown } from 'lucide-react'
import { cn } from '../utils'
import { Button } from './Button'

export type LogLevel = 'debug' | 'info' | 'warn' | 'error'

export interface LogEntry {
  id: number
  ts: number
  level: LogLevel
  text: string
}

export interface LogViewerProps {
  entries: LogEntry[]
  emptyText?: string
  jumpLabel?: string
  className?: string
}

const ROW_HEIGHT = 22
const levelStyle: Record<LogLevel, string> = {
  debug: 'text-fg-subtle',
  info: 'text-info',
  warn: 'text-warning',
  error: 'text-danger',
}
const timeFormat = new Intl.DateTimeFormat(undefined, {
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit',
  fractionalSecondDigits: 3,
  hour12: false,
})

/**
 * 实时日志：虚拟滚动，滚动到底部时自动跟随新日志；向上翻看时暂停跟随并提示跳到最新。
 */
export function LogViewer({ entries, emptyText, jumpLabel = 'Latest', className }: LogViewerProps) {
  const scrollRef = useRef<HTMLDivElement>(null)
  const [follow, setFollow] = useState(true)
  // TanStack Virtual 返回的函数无法被 React Compiler 记忆化，这里不依赖编译器优化
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: entries.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 20,
  })

  useLayoutEffect(() => {
    if (follow && entries.length > 0) virtualizer.scrollToIndex(entries.length - 1, { align: 'end' })
  }, [entries.length, follow, virtualizer])

  useEffect(() => {
    const el = scrollRef.current
    if (!el) return
    const onScroll = () => setFollow(el.scrollHeight - el.scrollTop - el.clientHeight < ROW_HEIGHT * 2)
    el.addEventListener('scroll', onScroll, { passive: true })
    return () => el.removeEventListener('scroll', onScroll)
  }, [])

  return (
    <div className={cn('relative h-full min-h-0 bg-surface-2', className)}>
      <div ref={scrollRef} className="h-full overflow-auto font-mono text-xs" role="log" aria-live="polite" data-selectable>
        {entries.length === 0 ? (
          <p className="p-4 text-fg-subtle">{emptyText}</p>
        ) : (
          <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
            {virtualizer.getVirtualItems().map((item) => {
              const entry = entries[item.index]
              return (
                <div
                  key={entry.id}
                  className="absolute left-0 flex w-full items-center gap-3 px-3 whitespace-pre hover:bg-hover"
                  style={{ height: ROW_HEIGHT, transform: `translateY(${item.start}px)` }}
                >
                  <span className="shrink-0 text-fg-subtle tabular-nums">{timeFormat.format(entry.ts)}</span>
                  <span className={cn('w-10 shrink-0 font-semibold uppercase', levelStyle[entry.level])}>{entry.level}</span>
                  <span className={cn('truncate', entry.level === 'error' ? 'text-danger' : 'text-fg')}>{entry.text}</span>
                </div>
              )
            })}
          </div>
        )}
      </div>
      {!follow && entries.length > 0 && (
        <Button
          size="sm"
          variant="secondary"
          className="absolute right-4 bottom-3 shadow-popover"
          onClick={() => {
            setFollow(true)
            virtualizer.scrollToIndex(entries.length - 1, { align: 'end' })
          }}
        >
          <ArrowDown />
          {jumpLabel}
        </Button>
      )}
    </div>
  )
}
