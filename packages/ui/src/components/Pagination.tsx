import { ChevronLeft, ChevronRight } from 'lucide-react'
import { cn } from '../utils'
import { Button } from './Button'

export interface PaginationProps {
  /** 当前页，从 1 开始 */
  page: number
  pageSize: number
  total: number
  onPageChange: (page: number) => void
  prevLabel: string
  nextLabel: string
  /** 页码按钮的无障碍标签，例如「第 3 页」 */
  pageLabel: (page: number) => string
  /** 总数说明，例如「共 128 条」；不传则不显示 */
  totalLabel?: string
  className?: string
  'aria-label'?: string
}

/** 页码窗口：总页数不超过 7 页全部展示，否则保留首末页与当前页 ±1，中间用 … 占位 */
function pageWindow(page: number, pages: number): (number | 'gap')[] {
  if (pages <= 7) return Array.from({ length: pages }, (_, i) => i + 1)
  const nums = new Set([1, pages, page - 1, page, page + 1])
  const list = [...nums].filter((n) => n >= 1 && n <= pages).sort((a, b) => a - b)
  const window: (number | 'gap')[] = []
  for (let i = 0; i < list.length; i++) {
    if (i > 0 && list[i] - list[i - 1] > 1) window.push('gap')
    window.push(list[i])
  }
  return window
}

export function Pagination({
  page,
  pageSize,
  total,
  onPageChange,
  prevLabel,
  nextLabel,
  pageLabel,
  totalLabel,
  className,
  ...aria
}: PaginationProps) {
  const pages = Math.max(1, Math.ceil(total / pageSize))
  if (total <= pageSize) return null

  return (
    <nav
      {...aria}
      className={cn('flex flex-wrap items-center justify-between gap-3', className)}
    >
      {totalLabel && <p className="text-xs text-fg-subtle tabular-nums">{totalLabel}</p>}
      <div className="flex items-center gap-1">
        <Button
          size="sm"
          variant="ghost"
          disabled={page <= 1}
          onClick={() => onPageChange(page - 1)}
          aria-label={prevLabel}
        >
          <ChevronLeft />
        </Button>
        {pageWindow(page, pages).map((item, index) =>
          item === 'gap' ? (
            <span key={`gap-${index}`} className="px-1 text-xs text-fg-subtle select-none">
              …
            </span>
          ) : (
            <Button
              key={item}
              size="sm"
              variant={item === page ? 'secondary' : 'ghost'}
              className="min-w-7 tabular-nums"
              aria-current={item === page ? 'page' : undefined}
              aria-label={pageLabel(item)}
              onClick={() => onPageChange(item)}
            >
              {item}
            </Button>
          ),
        )}
        <Button
          size="sm"
          variant="ghost"
          disabled={page >= pages}
          onClick={() => onPageChange(page + 1)}
          aria-label={nextLabel}
        >
          <ChevronRight />
        </Button>
      </div>
    </nav>
  )
}
