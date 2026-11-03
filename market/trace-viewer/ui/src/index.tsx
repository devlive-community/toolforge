import { useState } from 'react'
import { Badge, Panel, Spinner } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Activity } from 'lucide-react'

interface Event {
  name: string
  phase: string
  pid: number
  tid: number
  ts: number
  dur: number | null
  hasChildren: boolean
}
interface ProcessInfo {
  pid: number
  name: string
  threads: { tid: number; name: string; events: number }[]
}
interface Summary {
  name: string
  count: number
  totalMs: number
  maxMs: number
}
interface ParseOut {
  events: Event[]
  processes: ProcessInfo[]
  summaries: Summary[]
  unsupportedPhases: string[]
  diagnostics: string[]
  totalCount: number
}

const SAMPLE = JSON.stringify({ traceEvents: [
  { name: 'render', ph: 'X', pid: 1, tid: 1, ts: 100, dur: 50 },
  { name: 'layout', ph: 'X', pid: 1, tid: 1, ts: 110, dur: 30 },
] }) // tf-allow: 示例输入

export default function TraceViewer() {
  const { t, errorMessage } = usePlugin()
  const [json, setJson] = useState(SAMPLE) // tf-allow: 默认值
  const parse = useDebouncedCall<ParseOut>(json.trim() ? 'parse' : null, { json }, [json])

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <Panel icon={<Activity />} title={t('ui.input')} bodyClassName="p-3">
        <textarea
          value={json}
          onChange={(event) => setJson(event.target.value)}
          spellCheck={false}
          aria-label={t('ui.input')}
          className="h-28 w-full resize-none rounded-control border border-border bg-surface p-2 font-mono text-xs text-fg outline-none focus-visible:ring-2 focus-visible:ring-ring"
        />
      </Panel>
      <Panel icon={<Activity />} title={t('ui.events', { count: parse.result?.events.length ?? 0 })} className="min-h-0 flex-1" bodyClassName="min-h-0 overflow-auto space-y-1 p-3" extra={parse.pending && <Spinner className="size-3.5" />}>
        {parse.error ? (
          <p className="text-xs text-danger">{errorMessage(parse.error)}</p>
        ) : (
          parse.result && (
            <>
              <p className="text-[11px] text-fg-subtle">{t('ui.events', { count: parse.result.totalCount })}</p>
              {parse.result.processes.map((process) => (
                <div key={process.pid} className="rounded-control border border-border p-2">
                  <p className="text-xs font-medium text-fg">{process.name}</p>
                  <div className="mt-1 flex flex-wrap gap-1.5">
                    {process.threads.map((thread) => (
                      <Badge key={thread.tid} variant="neutral">{thread.name} ({thread.events})</Badge>
                    ))}
                  </div>
                </div>
              ))}
              {parse.result.summaries.length > 0 && (
                <div className="border-t border-border pt-2">
                  <p className="text-xs font-medium text-fg-muted">{t('ui.summaries')}</p>
                  {parse.result.summaries.slice(0, 20).map((summary) => (
                    <div key={summary.name} className="mt-1 flex items-center justify-between text-xs">
                      <span className="truncate font-mono text-fg">{summary.name}</span>
                      <span className="text-fg-muted tabular-nums">{t('ui.count')}{' '}{summary.count}{' · '}{t('ui.total')}{' '}{summary.totalMs.toFixed(1)}{'µs · '}{t('ui.max')}{' '}{summary.maxMs.toFixed(1)}{'µs'}</span>
                    </div>
                  ))}
                </div>
              )}
              {(parse.result.diagnostics.length > 0 || parse.result.unsupportedPhases.length > 0) && (
                <div className="rounded-control bg-warning-soft p-2 text-[11px] text-warning">
                  {parse.result.diagnostics.map((item) => <p key={item}>{item}</p>)}
                  {parse.result.unsupportedPhases.map((phase) => <p key={phase}>{t('ui.unsupported')}: {phase}</p>)}
                </div>
              )}
            </>
          )
        )}
      </Panel>
    </div>
  )
}
