import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { Empty, cn, toast } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { SearchX } from 'lucide-react'
import type { Level, Line, Page } from './types'

const BLOCK = 200
const ROW_HEIGHT = 22

export const LEVEL_BAR: Record<Level, string> = {
  error: 'bg-danger',
  warn: 'bg-warning',
  info: 'bg-info',
  debug: 'bg-fg-subtle',
  trace: 'bg-border-strong',
  none: 'bg-transparent',
}

const LEVEL_TEXT: Record<Level, string> = {
  error: 'text-danger',
  warn: 'text-warning',
  info: 'text-fg',
  debug: 'text-fg-muted',
  trace: 'text-fg-subtle',
  none: 'text-fg',
}

/** 高亮匹配片段（位置由 Rust 按 UTF-16 下标给出） */
function Marked({ line }: { line: Line }) {
  if (line.marks.length === 0) return <>{line.text}</>
  const parts: ReactNode[] = []
  let cursor = 0
  line.marks.forEach(([start, end], index) => {
    if (start > cursor) parts.push(line.text.slice(cursor, start))
    parts.push(
      <mark key={index} className="rounded-[2px] bg-warning-soft text-fg">
        {line.text.slice(start, end)}
      </mark>,
    )
    cursor = end
  })
  parts.push(line.text.slice(cursor))
  return <>{parts}</>
}

interface Block {
  /** 读取时的增长序号，用于判断块是否过期 */
  seq: number
  lines: Line[]
}

const EMPTY = new Map<number, Block>()

interface Blocks {
  key: string
  rows: Map<number, Block>
}

export interface Growth {
  /** 从这一行（视图中的下标）开始的块需要重新读取 */
  from: number
  seq: number
}

interface LogListProps {
  docId: number
  view: number
  /** 文件被截断 / 轮转后递增，丢弃全部已读取的块 */
  epoch: number
  total: number
  /** 文件总行数，决定行号栏宽度 */
  lineCount: number
  growth: Growth
  follow: boolean
  selected: number | null
  onSelect: (line: Line) => void
  onScrolledAway: () => void
}

export function LogList({ docId, view, epoch, total, lineCount, growth, follow, selected, onSelect, onScrolledAway }: LogListProps) {
  const { t, call, errorMessage } = usePlugin()
  const parent = useRef<HTMLDivElement>(null)
  const key = `${docId}:${view}:${epoch}`
  const [blocks, setBlocks] = useState<Blocks>({ key, rows: new Map() })
  if (blocks.key !== key) setBlocks({ key, rows: new Map() })
  const rows = blocks.key === key ? blocks.rows : EMPTY
  const pending = useRef(new Set<string>())
  // 文件增长后，包含增长起点的块及之后的块（末尾可能是不完整的行）需要重新读取
  const firstStale = Math.floor(Math.max(0, growth.from) / BLOCK)

  const ensure = useCallback(
    (block: number) => {
      const id = `${key}#${growth.seq}#${block}`
      if (pending.current.has(id)) return
      pending.current.add(id)
      const seq = growth.seq
      call<Page>('lines', { id: docId, view, offset: block * BLOCK, limit: BLOCK })
        .then((page) => {
          pending.current.delete(id)
          setBlocks((prev) => (prev.key === key ? { key, rows: new Map(prev.rows).set(block, { seq, lines: page.lines }) } : prev))
        })
        .catch((error) => {
          pending.current.delete(id)
          toast.error(errorMessage(error))
        })
    },
    [call, errorMessage, docId, view, key, growth.seq],
  )

  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: total,
    getScrollElement: () => parent.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 30,
  })
  const items = virtualizer.getVirtualItems()
  const first = items.length ? Math.floor(items[0].index / BLOCK) : 0
  const last = items.length ? Math.floor(items[items.length - 1].index / BLOCK) : 0

  useEffect(() => {
    for (let block = first; block <= last; block++) {
      const loaded = rows.get(block)
      const stale = loaded && loaded.seq < growth.seq && block >= firstStale
      if (!loaded || stale) ensure(block)
    }
  }, [first, last, rows, ensure, growth.seq, firstStale])

  // 切换文件或筛选后回到顶部；跟随模式滚到末尾
  useEffect(() => {
    parent.current?.scrollTo({ top: 0 })
  }, [key])
  useEffect(() => {
    if (follow && total > 0) virtualizer.scrollToIndex(total - 1, { align: 'end' })
  }, [follow, total, virtualizer])

  const width = String(Math.max(lineCount, 1)).length

  if (total === 0) return <Empty icon={<SearchX />} title={t('list.empty')} description={t('list.emptyHint')} />

  return (
    <div
      ref={parent}
      className="h-full overflow-auto font-mono text-[12px]"
      onWheel={(event) => follow && event.deltaY < 0 && onScrolledAway()}
    >
      <div className="relative min-w-full" style={{ height: virtualizer.getTotalSize() }}>
        {items.map((item) => {
          const line = rows.get(Math.floor(item.index / BLOCK))?.lines[item.index % BLOCK]
          return (
            <div
              key={item.key}
              onClick={() => line && onSelect(line)}
              className={cn(
                'absolute left-0 flex w-full cursor-default items-stretch hover:bg-hover',
                line && selected === line.n && 'bg-primary-soft hover:bg-primary-soft',
              )}
              style={{ top: item.start, height: ROW_HEIGHT }}
            >
              <span className={cn('w-0.5 shrink-0', line ? LEVEL_BAR[line.level] : 'bg-transparent')} />
              <span
                className="shrink-0 px-2 text-right text-fg-subtle tabular-nums select-none leading-[22px]"
                style={{ width: `${width + 2}ch` }}
              >
                {line?.n ?? ''}
              </span>
              <span className={cn('min-w-0 flex-1 truncate pr-3 whitespace-pre leading-[22px]', line ? LEVEL_TEXT[line.level] : '')}>
                {line ? <Marked line={line} /> : <span className="inline-block h-2 w-40 rounded-sm bg-active align-middle" />}
              </span>
            </div>
          )
        })}
      </div>
    </div>
  )
}
