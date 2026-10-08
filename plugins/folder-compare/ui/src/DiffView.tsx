import { useRef } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { Badge, Empty, Panel, cn } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { FileDiff } from 'lucide-react'
import type { Diff, DiffLine, DiffRow } from './types'

const ROW_HEIGHT = 20
const SIDE_BG: Record<DiffRow['tag'], [string, string]> = {
  equal: ['', ''],
  delete: ['bg-danger-soft', 'bg-surface-2'],
  insert: ['bg-surface-2', 'bg-success-soft'],
  change: ['bg-warning-soft', 'bg-warning-soft'],
}

function Cell({ line, className }: { line: DiffLine | null; className: string }) {
  return (
    <div className={cn('flex min-w-0', className)}>
      <span className="w-12 shrink-0 pr-2 text-right text-fg-subtle select-none">{line?.number ?? ''}</span>
      <span className="min-w-0 flex-1 truncate whitespace-pre text-fg" title={line?.text}>
        {line?.text ?? ''}
      </span>
    </div>
  )
}

export function DiffView({ left, right, path }: { left: string; right: string; path: string }) {
  const { t, errorMessage } = usePlugin()
  const args = { left, right, path }
  const diff = useDebouncedCall<Diff>('diff', args, [left, right, path])
  const listRef = useRef<HTMLDivElement>(null)
  const rows = diff.result?.rows ?? []
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({ count: rows.length, getScrollElement: () => listRef.current, estimateSize: () => ROW_HEIGHT, overscan: 30 })

  return (
    <Panel
      icon={<FileDiff />}
      title={path}
      extra={
        diff.result && (
          <>
            <Badge variant="success">+{diff.result.added}</Badge>
            <Badge variant="danger">−{diff.result.removed}</Badge>
            {diff.result.truncated && <Badge variant="warning">{t('diff.truncated')}</Badge>}
          </>
        )
      }
      className="min-h-0 flex-[2]"
      bodyClassName="p-0"
    >
      {diff.error ? (
        <Empty icon={<FileDiff />} title={errorMessage(diff.error)} />
      ) : rows.length === 0 && diff.result ? (
        <Empty icon={<FileDiff />} title={t('diff.identical')} />
      ) : (
        <div ref={listRef} className={cn('h-full overflow-auto font-mono text-[12px]', diff.pending && 'opacity-60')}>
          <div className="relative" style={{ height: virtualizer.getTotalSize() }}>
            {virtualizer.getVirtualItems().map((item) => {
              const row = rows[item.index]
              const [lb, rb] = SIDE_BG[row.tag]
              return (
                <div key={item.key} className="absolute left-0 grid w-full grid-cols-2 divide-x divide-border" style={{ top: item.start, height: ROW_HEIGHT }}>
                  <Cell line={row.left} className={cn('items-center', lb)} />
                  <Cell line={row.right} className={cn('items-center', rb)} />
                </div>
              )
            })}
          </div>
        </div>
      )}
    </Panel>
  )
}
