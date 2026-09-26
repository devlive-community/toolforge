import { useEffect, useState } from 'react'
import { Badge, Button, DropdownMenu, Empty, Input, Panel, Progress, Select, Tooltip, cn, toast } from '@toolforge/ui'
import { usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { ChevronDown, Play, Radar, Square, TriangleAlert } from 'lucide-react'
import type { CheckReport } from './types'

const PRESETS: { key: string; ports: string }[] = [
  { key: 'common', ports: '21,22,25,53,80,110,143,443,465,587,993,995,3306,3389,5432,6379,8080,8443' },
  { key: 'web', ports: '80,443,3000,5173,8000,8080,8443,8888' },
  { key: 'databases', ports: '1433,1521,3306,5432,6379,9200,11211,27017' },
]
const TIMEOUTS = [500, 1500, 3000, 5000]

/** 从 host:port 中拆出主机与端口 */
function splitEndpoint(text: string | undefined): { host: string; ports: string } | null {
  if (!text) return null
  const bracket = text.match(/^\[([^\]]+)\]:(\d+)$/)
  if (bracket) return { host: bracket[1], ports: bracket[2] }
  const index = text.lastIndexOf(':')
  return index > 0 ? { host: text.slice(0, index), ports: text.slice(index + 1) } : null
}

export function RemoteCheck({ endpoint }: { endpoint?: string }) {
  const { t, errorMessage } = usePlugin()
  const initial = splitEndpoint(endpoint)
  const [host, setHost] = useState(initial?.host ?? 'github.com')
  const [ports, setPorts] = useState(initial?.ports ?? '22,80,443')
  const [timeout, setTimeoutMs] = useState(1500)
  const task = useTask<CheckReport>()
  const report = task.result

  useEffect(() => {
    if (task.status === 'failed' && task.error && task.error.code !== 'task.cancelled') toast.error(errorMessage(task.error))
  }, [task.status, task.error, errorMessage])

  const start = () => task.start('check', { host, ports, timeoutMs: timeout })
  const progress = task.progress && task.progress.total > 0 ? (task.progress.done / task.progress.total) * 100 : null

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <section className="flex flex-wrap items-end gap-3 rounded-card border border-border bg-surface p-3 shadow-card">
        <label className="flex flex-col gap-1.5">
          <span className="text-xs font-medium text-fg-muted">{t('remote.host')}</span>
          <Input value={host} onChange={(event) => setHost(event.target.value)} className="font-mono" wrapperClassName="w-64" aria-label={t('remote.host')} spellCheck={false} />
        </label>
        <label className="flex min-w-0 flex-1 flex-col gap-1.5">
          <span className="flex items-center justify-between text-xs font-medium text-fg-muted">
            {t('remote.ports')}
            <DropdownMenu
              trigger={
                <button type="button" className="flex items-center gap-0.5 rounded-sm text-xs text-primary outline-none hover:underline focus-visible:ring-2 focus-visible:ring-ring">
                  {t('remote.presets')}
                  <ChevronDown className="size-3" />
                </button>
              }
              items={PRESETS.map((preset) => ({ key: preset.key, label: t(`presets.${preset.key}`), onSelect: () => setPorts(preset.ports) }))}
            />
          </span>
          <Input
            value={ports}
            onChange={(event) => setPorts(event.target.value)}
            onKeyDown={(event) => event.key === 'Enter' && !task.running && start()}
            placeholder={t('remote.portsHint')}
            className="font-mono"
            aria-label={t('remote.ports')}
            spellCheck={false}
          />
        </label>
        <label className="flex flex-col gap-1.5">
          <span className="text-xs font-medium text-fg-muted">{t('remote.timeout')}</span>
          <Select<number>
            value={timeout}
            onValueChange={setTimeoutMs}
            className="w-28"
            aria-label={t('remote.timeout')}
            options={TIMEOUTS.map((ms) => ({ value: ms, label: t('remote.ms', { ms }) }))}
          />
        </label>
        {task.running ? (
          <Button onClick={task.cancel}>
            <Square />
            {t('remote.cancel')}
          </Button>
        ) : (
          <Button variant="primary" onClick={start} disabled={!host.trim() || !ports.trim()}>
            <Play />
            {t('remote.check')}
          </Button>
        )}
      </section>

      <Panel
        icon={<Radar />}
        title={report ? t('remote.resultTitle', { host: report.host, address: report.address }) : t('remote.title')}
        extra={report && <Badge variant={report.open > 0 ? 'success' : 'neutral'}>{t('remote.summary', { open: report.open, total: report.results.length })}</Badge>}
        footer={report && <span className="tabular-nums">{t('remote.elapsed', { ms: report.elapsedMs })}</span>}
        bodyClassName="overflow-auto p-3"
      >
        {task.running && (
          <div className="mb-3 space-y-1.5">
            <Progress value={progress} />
            <p className="text-xs text-fg-muted tabular-nums">{t('remote.progress', { done: task.progress?.done ?? 0, total: task.progress?.total ?? 0 })}</p>
          </div>
        )}
        {report?.fakeIp && (
          <p className="mb-3 flex items-start gap-1.5 rounded-control bg-warning-soft px-2.5 py-2 text-xs text-warning">
            <TriangleAlert className="mt-px size-3.5 shrink-0" />
            {t('remote.fakeIp', { address: report.address })}
          </p>
        )}
        {report ? (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(150px,1fr))] gap-2">
            {report.results.map((result) => (
              <Tooltip key={result.port} content={result.open ? t('remote.openHint') : t(`reasons.${result.reason ?? 'other'}`)}>
                <div
                  className={cn(
                    'flex items-center justify-between rounded-control border px-3 py-2',
                    result.open ? 'border-success/40 bg-success-soft' : 'border-border bg-surface-2',
                  )}
                >
                  <span className={cn('font-mono text-[13px] font-semibold tabular-nums', result.open ? 'text-success' : 'text-fg-muted')}>{result.port}</span>
                  <span className="text-xs text-fg-muted tabular-nums">
                    {result.open ? t('remote.ms', { ms: result.ms }) : t(`reasonsShort.${result.reason ?? 'other'}`)}
                  </span>
                </div>
              </Tooltip>
            ))}
          </div>
        ) : (
          !task.running && <Empty icon={<Radar />} title={t('remote.placeholder')} description={t('remote.placeholderHint')} />
        )}
      </Panel>
    </div>
  )
}
