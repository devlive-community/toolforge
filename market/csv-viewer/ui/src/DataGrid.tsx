import { useCallback, useEffect, useRef, useState } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { Empty, Spinner, Tooltip, cn, toast } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { ArrowDown, ArrowUp, Calendar, Hash, PanelRight, SearchX, ToggleLeft, Type } from 'lucide-react'
import type { Kind, Page, Row, Spec, TableInfo } from './types'

const BLOCK = 200
const ROW_HEIGHT = 30
const INDEX_WIDTH = 72
const COLUMN_WIDTH = 180

const KIND_ICON: Record<Kind, typeof Hash> = {
  empty: Type,
  integer: Hash,
  float: Hash,
  boolean: ToggleLeft,
  date: Calendar,
  text: Type,
}

interface Blocks {
  key: string
  total: number | null
  rows: Map<number, Row[]>
}

/** 按块读取当前视图的行；视图变化时丢弃旧块 */
function useBlocks(info: TableInfo, spec: Spec) {
  const { call, errorMessage } = usePlugin()
  const key = `${info.id}:${JSON.stringify(spec)}`
  const [blocks, setBlocks] = useState<Blocks>({ key, total: null, rows: new Map() })
  if (blocks.key !== key) setBlocks({ key, total: null, rows: new Map() })
  const pending = useRef(new Set<string>())

  const ensure = useCallback(
    (block: number) => {
      const id = `${key}#${block}`
      if (pending.current.has(id)) return
      pending.current.add(id)
      call<Page>('rows', { id: info.id, offset: block * BLOCK, limit: BLOCK, ...spec })
        .then((page) =>
          setBlocks((prev) => (prev.key === key ? { ...prev, total: page.total, rows: new Map(prev.rows).set(block, page.rows) } : prev)),
        )
        .catch((error) => {
          pending.current.delete(id)
          toast.error(errorMessage(error))
        })
    },
    [call, errorMessage, info.id, key, spec],
  )

  const current = blocks.key === key ? blocks : { key, total: null, rows: new Map<number, Row[]>() }
  return { total: current.total, rows: current.rows, ensure }
}

interface DataGridProps {
  info: TableInfo
  spec: Spec
  selected: number | null
  onSort: (column: number) => void
  onSelect: (column: number) => void
  onTotal: (total: number | null) => void
  columnName: (column: number) => string
}

export function DataGrid({ info, spec, selected, onSort, onSelect, onTotal, columnName }: DataGridProps) {
  const { t } = usePlugin()
  const parent = useRef<HTMLDivElement>(null)
  const { total, rows, ensure } = useBlocks(info, spec)
  const width = INDEX_WIDTH + COLUMN_WIDTH * info.columns.length

  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: total ?? 0,
    getScrollElement: () => parent.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 20,
  })
  const items = virtualizer.getVirtualItems()
  const first = items.length ? Math.floor(items[0].index / BLOCK) : 0
  const last = items.length ? Math.floor(items[items.length - 1].index / BLOCK) : 0

  useEffect(() => {
    for (let block = first; block <= last; block++) if (!rows.has(block)) ensure(block)
  }, [first, last, rows, ensure])

  useEffect(() => onTotal(total), [total, onTotal])

  // 视图变化后回到顶部
  const key = `${info.id}:${JSON.stringify(spec)}`
  useEffect(() => {
    parent.current?.scrollTo({ top: 0 })
  }, [key])

  const sortOf = (column: number) => spec.sort.find((s) => s.column === column)

  return (
    <div ref={parent} className="relative h-full overflow-auto text-[12.5px]">
      <div style={{ width, minWidth: '100%' }}>
        <div className="sticky top-0 z-10 flex border-b border-border bg-surface-2" style={{ height: ROW_HEIGHT + 4 }}>
          <div className="sticky left-0 z-10 shrink-0 border-r border-border bg-surface-2" style={{ width: INDEX_WIDTH }} />
          {info.columns.map((column, index) => {
            const Icon = KIND_ICON[column.kind]
            const sort = sortOf(index)
            return (
              <div
                key={index}
                className={cn('group flex shrink-0 items-center border-r border-border', selected === index && 'bg-primary-soft')}
                style={{ width: COLUMN_WIDTH }}
              >
                <button
                  type="button"
                  onClick={() => onSort(index)}
                  title={columnName(index)}
                  className="flex h-full min-w-0 flex-1 items-center gap-1.5 px-2.5 text-left font-semibold text-fg outline-none hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset"
                >
                  <Icon className="size-3.5 shrink-0 text-fg-subtle" aria-label={t(`kinds.${column.kind}`)} />
                  <span className="truncate">{columnName(index)}</span>
                  {sort &&
                    (sort.desc ? (
                      <ArrowDown className="size-3.5 shrink-0 text-primary" aria-label={t('table.sortDesc')} />
                    ) : (
                      <ArrowUp className="size-3.5 shrink-0 text-primary" aria-label={t('table.sortAsc')} />
                    ))}
                </button>
                <Tooltip content={t('table.details')}>
                  <button
                    type="button"
                    onClick={() => onSelect(index)}
                    aria-label={t('table.details')}
                    className={cn(
                      'mr-1 flex size-6 shrink-0 items-center justify-center rounded-control text-fg-subtle outline-none hover:bg-hover hover:text-fg focus-visible:ring-2 focus-visible:ring-ring',
                      selected === index ? 'text-primary' : 'opacity-0 group-hover:opacity-100 focus-visible:opacity-100',
                    )}
                  >
                    <PanelRight className="size-3.5" />
                  </button>
                </Tooltip>
              </div>
            )
          })}
        </div>

        {total === null ? (
          <div className="flex h-40 items-center justify-center">
            <Spinner />
          </div>
        ) : total === 0 ? (
          <Empty icon={<SearchX />} title={t('table.empty')} className="sticky left-0 h-60 w-full max-w-[calc(100vw-20rem)]" />
        ) : (
          <div className="relative" style={{ height: virtualizer.getTotalSize() }}>
            {items.map((item) => {
              const row = rows.get(Math.floor(item.index / BLOCK))?.[item.index % BLOCK]
              return (
                <div
                  key={item.key}
                  className="absolute left-0 flex border-b border-border hover:bg-hover"
                  style={{ top: item.start, height: ROW_HEIGHT, width }}
                >
                  <div
                    className="sticky left-0 shrink-0 border-r border-border bg-surface-2 px-2.5 text-right leading-[30px] text-fg-subtle tabular-nums"
                    style={{ width: INDEX_WIDTH }}
                  >
                    {row ? row.index + 1 : ''}
                  </div>
                  {info.columns.map((column, index) => {
                    const value = row?.cells[index] ?? ''
                    const numeric = column.kind === 'integer' || column.kind === 'float'
                    return (
                      <div
                        key={index}
                        title={value.length > 24 ? value : undefined}
                        data-selectable
                        className={cn(
                          'shrink-0 truncate border-r border-border px-2.5 text-fg',
                          numeric && 'text-right font-mono text-[12px] tabular-nums',
                          selected === index && 'bg-primary-soft/40',
                          // 放在字号之后，避免被 tailwind-merge 覆盖
                          'leading-[30px]',
                        )}
                        style={{ width: COLUMN_WIDTH }}
                      >
                        {row ? value : <span className="inline-block h-2 w-16 rounded-sm bg-active align-middle" />}
                      </div>
                    )
                  })}
                </div>
              )
            })}
          </div>
        )}
      </div>
    </div>
  )
}
