import { Badge, Input, Panel, Select, cn } from '@toolforge/ui'
import { formatBytes, usePlugin } from '@toolforge/plugin-ui-sdk'
import { ListTree, Search } from 'lucide-react'
import type { Snapshot, SortBy } from './types'

interface Props {
  snapshot: Snapshot
  sort: SortBy
  onSortChange: (sort: SortBy) => void
  query: string
  onQueryChange: (query: string) => void
}

export function ProcessList({ snapshot, sort, onSortChange, query, onQueryChange }: Props) {
  const { t } = usePlugin()
  const processes = snapshot.processes
  const cores = Math.max(1, snapshot.cpu.logicalCores)

  return (
    <Panel
      icon={<ListTree />}
      title={t('processes.title')}
      extra={processes && <Badge>{t('processes.count', { matched: processes.matched, total: processes.total })}</Badge>}
      actions={
        <>
          <Input
            size="sm"
            value={query}
            onChange={(event) => onQueryChange(event.target.value)}
            leading={<Search />}
            placeholder={t('processes.search')}
            aria-label={t('processes.search')}
            wrapperClassName="w-56"
          />
          <Select<SortBy>
            size="sm"
            value={sort}
            onValueChange={onSortChange}
            className="w-36"
            aria-label={t('processes.sort')}
            options={(['cpu', 'memory', 'name', 'pid'] as const).map((value) => ({ value, label: t(`processes.sortBy.${value}`) }))}
          />
        </>
      }
      className="h-full"
      bodyClassName="overflow-auto"
    >
      <table className="w-full text-left text-xs">
        <thead className="sticky top-0 z-10 bg-surface-2 text-fg-muted">
          <tr>
            <th className="px-3 py-2 font-medium">{t('processes.pid')}</th>
            <th className="px-3 py-2 font-medium">{t('processes.name')}</th>
            <th className="px-3 py-2 text-right font-medium">{t('processes.cpu')}</th>
            <th className="px-3 py-2 text-right font-medium">{t('processes.memory')}</th>
            <th className="px-3 py-2 font-medium">{t('processes.user')}</th>
            <th className="px-3 py-2 font-medium">{t('processes.status')}</th>
          </tr>
        </thead>
        <tbody>
          {processes?.list.map((p) => {
            // sysinfo 的进程 CPU 占用以单核为 100%，换算为整机占比
            const cpu = p.cpu / cores
            return (
              <tr key={p.pid} className="border-t border-border hover:bg-hover">
                <td className="px-3 py-1.5 font-mono text-fg-muted tabular-nums" data-selectable>
                  {p.pid}
                </td>
                <td className="max-w-80 truncate px-3 py-1.5 text-fg" title={p.exe ?? p.name} data-selectable>
                  {p.name}
                </td>
                <td className={cn('px-3 py-1.5 text-right tabular-nums', cpu >= 50 ? 'text-danger' : cpu >= 10 ? 'text-warning' : 'text-fg')}>{`${cpu.toFixed(1)}%`}</td>
                <td className="px-3 py-1.5 text-right text-fg tabular-nums">{formatBytes(p.memory)}</td>
                <td className="max-w-32 truncate px-3 py-1.5 text-fg-muted">{p.user ?? '-'}</td>
                <td className="px-3 py-1.5 text-fg-muted">{p.status}</td>
              </tr>
            )
          })}
        </tbody>
      </table>
      {processes && processes.list.length === 0 && <p className="p-6 text-center text-xs text-fg-subtle">{t('processes.empty')}</p>}
    </Panel>
  )
}
