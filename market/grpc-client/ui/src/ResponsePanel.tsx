import { useState } from 'react'
import { Badge, Button, DropdownMenu, Empty, Panel, Spinner, Tabs } from '@toolforge/ui'
import { formatBytes, usePlugin } from '@toolforge/plugin-ui-sdk'
import { ArrowDownToLine, Copy, Inbox, Radio } from 'lucide-react'
import type { Run } from './types'

/** 只渲染最近这么多条消息，全部消息可导出 */
const SHOWN = 200

type ResponseTab = 'messages' | 'headers' | 'trailers'

function Pairs({ pairs, empty }: { pairs: [string, string][]; empty: string }) {
  if (pairs.length === 0) return <p className="p-3 text-xs text-fg-subtle">{empty}</p>
  return (
    <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 p-3 text-xs">
      {pairs.map(([k, v], i) => (
        <div key={`${k}-${i}`} className="contents">
          <dt className="font-mono text-fg-muted">{k}</dt>
          <dd className="font-mono break-all text-fg" data-selectable>
            {v}
          </dd>
        </div>
      ))}
    </dl>
  )
}

export function ResponsePanel({ run, onExport }: { run: Run | null; onExport: (copy: boolean) => void }) {
  const { t } = usePlugin()
  const [tab, setTab] = useState<ResponseTab>('messages')
  const batch = run?.batch ?? null
  const status = batch?.status ?? null
  const shown = run ? run.messages.slice(-SHOWN) : []
  const offset = run ? run.messages.length - shown.length : 0

  return (
    <Panel
      title={
        <Tabs<ResponseTab>
          variant="underline"
          value={tab}
          onValueChange={setTab}
          aria-label={t('response.title')}
          items={[
            { value: 'messages', label: t('response.messages', { count: run?.messages.length ?? 0 }) },
            { value: 'headers', label: t('response.headers', { count: run?.headers.length ?? 0 }) },
            { value: 'trailers', label: t('response.trailers', { count: batch?.trailers.length ?? 0 }) },
          ]}
        />
      }
      actions={
        run?.call ? (
          <DropdownMenu
            trigger={
              <Button size="sm" variant="ghost" disabled={run.running}>
                <ArrowDownToLine />
                {t('export.title')}
              </Button>
            }
            items={[
              { key: 'copy', label: t('export.copy'), icon: <Copy />, onSelect: () => onExport(true) },
              { key: 'save', label: t('export.save'), icon: <ArrowDownToLine />, onSelect: () => onExport(false) },
            ]}
          />
        ) : null
      }
      footer={
        run && (
          <>
            {status ? (
              <Badge variant={status.code === 0 ? 'success' : 'danger'}>
                {status.name} ({status.code})
              </Badge>
            ) : (
              <span className="flex items-center gap-1.5">
                <Spinner className="size-3" />
                {t('response.receiving')}
              </span>
            )}
            {status?.message && <span className="min-w-0 truncate text-fg" data-selectable>{status.message}</span>}
            {status?.local && <span>{t('response.local')}</span>}
            <span className="ml-auto shrink-0 tabular-nums">
              {t('response.stats', { count: batch?.count ?? 0, size: formatBytes(batch?.bytes ?? 0), ms: batch?.elapsedMs ?? 0 })}
            </span>
            {batch?.truncated && <span className="shrink-0 text-warning">{t('response.truncated')}</span>}
          </>
        )
      }
      className="min-h-0"
      bodyClassName="min-h-0 overflow-auto"
    >
      {!run ? (
        <Empty icon={<Inbox />} title={t('response.empty')} description={t('response.emptyHint')} className="h-full" />
      ) : tab === 'messages' ? (
        run.messages.length === 0 ? (
          <Empty
            icon={run.running ? <Radio /> : <Inbox />}
            title={run.running ? t('response.waiting') : t('response.noMessages')}
            className="h-full"
          />
        ) : (
          <div className="space-y-2 p-3">
            {offset > 0 && <p className="text-xs text-fg-subtle">{t('response.older', { count: offset })}</p>}
            {shown.map((message, i) => (
              <div key={offset + i} className="rounded-control border border-border bg-surface-2">
                <div className="border-b border-border px-2.5 py-1 text-[11px] text-fg-subtle">#{offset + i + 1}</div>
                <pre className="overflow-x-auto p-2.5 font-mono text-xs leading-relaxed text-fg" data-selectable>
                  {message}
                </pre>
              </div>
            ))}
          </div>
        )
      ) : tab === 'headers' ? (
        <Pairs pairs={run.headers} empty={t('response.noHeaders')} />
      ) : (
        <Pairs pairs={batch?.trailers ?? []} empty={t('response.noTrailers')} />
      )}
    </Panel>
  )
}
