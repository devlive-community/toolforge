import { useState } from 'react'
import { Badge, Empty, Input, Panel, Spinner } from '@toolforge/ui'
import { CopyButton, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { ArrowRightLeft, ListTree } from 'lucide-react'
import type { RangeReport } from './types'

export function RangeTool() {
  const { t, errorMessage } = usePlugin()
  const [start, setStart] = useState('10.0.0.5')
  const [end, setEnd] = useState('10.0.1.20')

  const ready = start.trim() && end.trim()
  const args = ready ? { start, end } : null
  const { result, error, pending } = useDebouncedCall<RangeReport>(ready ? 'range_to_cidrs' : null, args, [start, end])
  const report = error ? null : result

  return (
    <div className="grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)] gap-3">
      <section className="space-y-2 rounded-card border border-border bg-surface p-4 shadow-card">
        <div className="grid grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-center gap-3">
          <Input value={start} onChange={(event) => setStart(event.target.value)} className="font-mono" placeholder={t('range.start')} aria-label={t('range.start')} />
          <ArrowRightLeft className="size-4 text-fg-subtle" />
          <Input value={end} onChange={(event) => setEnd(event.target.value)} className="font-mono" placeholder={t('range.end')} aria-label={t('range.end')} />
        </div>
        <p className="text-xs text-fg-subtle">{t('range.hint')}</p>
        {error && <p className="text-xs text-danger">{errorMessage(error)}</p>}
      </section>

      <Panel
        icon={<ListTree />}
        title={t('range.result')}
        extra={pending ? <Spinner className="size-3.5 text-fg-subtle" /> : report && <Badge>{t('range.summary', { count: report.cidrs.length, total: report.total })}</Badge>}
        actions={report && <CopyButton text={report.cidrs.join('\n')} label={t('range.copyAll')} variant="outline" />}
        bodyClassName="overflow-auto p-2"
      >
        {!report ? (
          <Empty icon={<ListTree />} title={t('range.empty')} />
        ) : (
          <ul className="font-mono text-[13px]" data-selectable>
            {report.cidrs.map((cidr) => (
              <li key={cidr} className="group flex items-center gap-3 rounded-control px-3 py-1.5 hover:bg-hover">
                <span className="min-w-0 flex-1 text-fg">{cidr}</span>
                <CopyButton text={cidr} className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100" />
              </li>
            ))}
          </ul>
        )}
      </Panel>
    </div>
  )
}
