import { useEffect, useState } from 'react'
import { Spinner, Tabs } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { Gauge, ListTree } from 'lucide-react'
import { Overview } from './Overview'
import { ProcessList } from './ProcessList'
import type { Snapshot, SortBy } from './types'

const INTERVAL = 2000
const HISTORY = 60

function SystemMonitor() {
  const { t, call, errorMessage } = usePlugin()
  const [tab, setTab] = useState<'overview' | 'processes'>('overview')
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null)
  const [history, setHistory] = useState<{ cpu: number[]; memory: number[] }>({ cpu: [], memory: [] })
  const [error, setError] = useState<string | null>(null)
  const [sort, setSort] = useState<SortBy>('cpu')
  const [query, setQuery] = useState('')

  // 定时向 Rust 采样；切换页签、排序或搜索时立即刷新
  useEffect(() => {
    let alive = true
    const tick = () =>
      call<Snapshot>('snapshot', { processes: tab === 'processes', sort, query, limit: 200 }).then(
        (next) => {
          if (!alive) return
          setSnapshot(next)
          setError(null)
          const memory = next.memory.total > 0 ? (next.memory.used / next.memory.total) * 100 : 0
          setHistory((prev) => ({
            cpu: [...prev.cpu, next.cpu.usage].slice(-HISTORY),
            memory: [...prev.memory, memory].slice(-HISTORY),
          }))
        },
        (reason) => alive && setError(errorMessage(reason)),
      )
    tick()
    const timer = setInterval(tick, INTERVAL)
    return () => {
      alive = false
      clearInterval(timer)
    }
  }, [call, errorMessage, tab, sort, query])

  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      <div className="flex items-center gap-3">
        <Tabs
          value={tab}
          onValueChange={setTab}
          items={[
            { value: 'overview', label: t('tabs.overview'), icon: <Gauge /> },
            { value: 'processes', label: t('tabs.processes'), icon: <ListTree /> },
          ]}
          aria-label={t('name')}
        />
        <span className="text-xs text-fg-subtle">{t('refresh', { seconds: INTERVAL / 1000 })}</span>
      </div>
      <div className="min-h-0 flex-1">
        {error && !snapshot ? (
          <p className="p-4 text-[13px] text-danger">{error}</p>
        ) : !snapshot ? (
          <div className="flex h-full items-center justify-center text-fg-muted">
            <Spinner />
          </div>
        ) : tab === 'overview' ? (
          <Overview snapshot={snapshot} history={history} />
        ) : (
          <ProcessList snapshot={snapshot} sort={sort} onSortChange={setSort} query={query} onQueryChange={setQuery} />
        )}
      </div>
    </div>
  )
}

export default SystemMonitor
