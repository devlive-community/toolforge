import { useState, type ReactNode } from 'react'
import { Badge, Empty, Input, Pagination, SegmentedControl, Spinner, cn } from '@toolforge/ui'
import { formatBytes, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Search } from 'lucide-react'
import { functionLabel, hex } from './format'
import type { ExportRow, FunctionRow, ImportRow, ListKind } from './types'
import { PAGE_SIZE, usePaged } from './usePaged'

interface Column<T> {
  key: string
  label: string
  className?: string
  render: (row: T) => ReactNode
}

function PagedTable<T>({
  token,
  kind,
  columns,
  rowKey,
  sort,
  toolbar,
  onRowClick,
}: {
  token: number
  kind: ListKind
  columns: Column<T>[]
  rowKey: (row: T) => string
  sort?: string
  toolbar?: ReactNode
  onRowClick?: (row: T) => void
}) {
  const { t } = usePlugin()
  const [query, setQuery] = useState('')
  const [page, setPage] = useState(0)
  const { page: data, stale, error } = usePaged<T>(token, kind, query, page, sort)

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <Input
          value={query}
          onChange={(e) => {
            setQuery(e.target.value)
            setPage(0)
          }}
          placeholder={t('list.search')}
          aria-label={t('list.search')}
          wrapperClassName="w-72"
          leading={<Search className="size-3.5" />}
        />
        {toolbar}
        {data && <span className="ml-auto text-xs text-fg-muted">{t('list.total', { count: data.total })}</span>}
      </div>
      <div className="min-h-0 flex-1 overflow-auto rounded-card border border-border bg-surface">
        {error ? (
          <p className="p-4 text-[13px] text-danger">{error}</p>
        ) : !data ? (
          <div className="flex h-32 items-center justify-center">
            <Spinner />
          </div>
        ) : data.items.length === 0 ? (
          <Empty title={query ? t('list.noMatch') : t(`list.empty.${kind}`)} className="h-full" />
        ) : (
          <table className={cn('w-full text-[13px]', stale && 'opacity-60')}>
            <thead className="sticky top-0 bg-surface-2">
              <tr className="border-b border-border text-left text-xs text-fg-muted">
                {columns.map((c) => (
                  <th key={c.key} className={cn('px-3 py-2 font-medium whitespace-nowrap', c.className)}>
                    {c.label}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {data.items.map((row) => (
                <tr
                  key={rowKey(row)}
                  className={cn('border-b border-border last:border-0', onRowClick && 'cursor-pointer hover:bg-hover')}
                  onClick={onRowClick ? () => onRowClick(row) : undefined}
                >
                  {columns.map((c) => (
                    <td key={c.key} className={cn('px-3 py-1.5 align-top', c.className)}>
                      {c.render(row)}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
      {data && data.total > PAGE_SIZE && (
        <Pagination
          page={page}
          pageSize={PAGE_SIZE}
          total={data.total}
          onPageChange={setPage}
          prevLabel={t('list.prev')}
          nextLabel={t('list.next')}
          pageLabel={(p) => t('list.page', { page: p })}
        />
      )}
    </div>
  )
}

const mono = 'font-mono text-xs break-all'

function KindBadge({ kind }: { kind: string }) {
  return <Badge variant={kind === 'func' ? 'primary' : kind === 'memory' ? 'info' : 'neutral'}>{kind}</Badge>
}

export function Imports({ token }: { token: number }) {
  const { t } = usePlugin()
  return (
    <PagedTable<ImportRow>
      token={token}
      kind="imports"
      rowKey={(r) => `${r.kind}-${r.index}`}
      columns={[
        { key: 'kind', label: t('list.kind'), className: 'w-20', render: (r) => <KindBadge kind={r.kind} /> },
        { key: 'module', label: t('list.module'), render: (r) => <span className={mono} data-selectable>{r.module}</span> },
        { key: 'name', label: t('list.name'), render: (r) => <span className={mono} data-selectable>{r.name}</span> },
        { key: 'desc', label: t('list.type'), render: (r) => <span className={cn(mono, 'text-fg-muted')}>{r.desc}</span> },
        { key: 'index', label: t('list.index'), className: 'w-16 text-right', render: (r) => <span className="tabular-nums text-fg-muted">{r.index}</span> },
      ]}
    />
  )
}

export function Exports({ token }: { token: number }) {
  const { t } = usePlugin()
  return (
    <PagedTable<ExportRow>
      token={token}
      kind="exports"
      rowKey={(r) => r.name}
      columns={[
        { key: 'kind', label: t('list.kind'), className: 'w-20', render: (r) => <KindBadge kind={r.kind} /> },
        { key: 'name', label: t('list.name'), render: (r) => <span className={mono} data-selectable>{r.name}</span> },
        { key: 'desc', label: t('list.type'), render: (r) => <span className={cn(mono, 'text-fg-muted')}>{r.desc}</span> },
        { key: 'index', label: t('list.index'), className: 'w-16 text-right', render: (r) => <span className="tabular-nums text-fg-muted">{r.index}</span> },
      ]}
    />
  )
}

export function Functions({ token, codeSize, onOpen }: { token: number; codeSize: number; onOpen: (f: FunctionRow) => void }) {
  const { t } = usePlugin()
  const [sort, setSort] = useState<'index' | 'size'>('index')
  return (
    <PagedTable<FunctionRow>
      token={token}
      kind="functions"
      sort={sort}
      rowKey={(r) => String(r.index)}
      onRowClick={onOpen}
      toolbar={
        <SegmentedControl
          size="sm"
          value={sort}
          onValueChange={setSort}
          options={[
            { value: 'index', label: t('list.byIndex') },
            { value: 'size', label: t('list.bySize') },
          ]}
          aria-label={t('list.sort')}
        />
      }
      columns={[
        { key: 'index', label: t('list.index'), className: 'w-16 tabular-nums text-fg-muted', render: (r) => r.index },
        {
          key: 'name',
          label: t('list.name'),
          render: (r) => (
            <div className="min-w-0">
              <div className={cn(mono, 'text-fg')}>{functionLabel(r)}</div>
              {r.export && r.name && r.export !== r.name && (
                <div className="text-xs text-fg-subtle">{t('list.exportedAs', { name: r.export })}</div>
              )}
            </div>
          ),
        },
        { key: 'sig', label: t('list.signature'), render: (r) => <span className={cn(mono, 'text-fg-muted')}>{r.signature}</span> },
        { key: 'locals', label: t('list.locals'), className: 'w-16 text-right tabular-nums text-fg-muted', render: (r) => r.locals },
        {
          key: 'size',
          label: t('list.size'),
          className: 'w-36 text-right',
          render: (r) => (
            <span className="tabular-nums text-fg-muted">
              {formatBytes(r.size)}
              {codeSize > 0 && <span className="ml-1 text-fg-subtle">{((r.size / codeSize) * 100).toFixed(1)}%</span>}
            </span>
          ),
        },
        { key: 'offset', label: t('list.offset'), className: 'w-24 text-right font-mono text-xs text-fg-subtle', render: (r) => hex(r.offset) },
      ]}
    />
  )
}
