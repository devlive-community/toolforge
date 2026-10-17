import { useState } from 'react'
import { Badge, Button, DropdownMenu, Empty, Input, Pagination, Panel, Spinner, Tabs, toast } from '@toolforge/ui'
import { CopyButton, formatBytes, host, usePlugin } from '@toolforge/plugin-ui-sdk'
import { ArrowDownToLine, Copy, FileWarning, Radio, RotateCw, Search } from 'lucide-react'
import type { StreamItem } from './types'
import type { StreamRun } from './useStream'

const PAGE_SIZE = 50

type StreamTab = 'events' | 'headers'

function statusTone(status: number) {
  if (status < 300) return 'success' as const
  if (status < 400) return 'warning' as const
  return 'danger' as const
}

function seconds(ms: number) {
  return (ms / 1000).toFixed(ms < 10_000 ? 2 : 1)
}

function EventCard({ item }: { item: StreamItem }) {
  const { t } = usePlugin()
  return (
    <article className="rounded-control border border-border bg-surface">
      <header className="flex items-center gap-2 border-b border-border px-3 py-1.5 text-xs">
        <span className="font-mono text-fg-subtle">#{item.seq}</span>
        {item.kind === 'chunk' ? (
          <Badge>{t('stream.chunk')}</Badge>
        ) : (
          <Badge variant={item.event ? 'primary' : 'neutral'}>{item.event ?? 'message'}</Badge>
        )}
        {item.id !== undefined && <span className="truncate font-mono text-fg-muted" data-selectable>{t('stream.id', { id: item.id })}</span>}
        {item.retry !== undefined && <span className="shrink-0 text-fg-subtle">{t('stream.retry', { ms: item.retry })}</span>}
        {item.truncated && <Badge variant="warning">{t('stream.truncated')}</Badge>}
        <span className="ml-auto shrink-0 font-mono text-fg-subtle tabular-nums">{t('stream.at', { seconds: seconds(item.atMs) })}</span>
        <CopyButton text={item.data} size="icon-sm" />
      </header>
      <pre className="max-h-64 overflow-auto px-3 py-2 font-mono text-xs break-all whitespace-pre-wrap text-fg" data-selectable>
        {item.data === '' ? <span className="text-fg-subtle">{t('stream.emptyData')}</span> : (item.formatted ?? item.data)}
      </pre>
    </article>
  )
}

export function StreamPanel({ run, onExport, onResume }: { run: StreamRun; onExport: (format: 'json' | 'sse') => Promise<string>; onResume: (lastEventId: string) => void }) {
  const { t, errorMessage } = usePlugin()
  const [tab, setTab] = useState<StreamTab>('events')
  const [query, setQuery] = useState('')
  const [page, setPage] = useState(1)
  const { opened, batch, events } = run
  const needle = query.trim().toLowerCase()
  // 最新的在前；搜索匹配事件类型、id 与数据
  const matched = (needle ? events.filter((e) => (e.event ?? 'message').toLowerCase().includes(needle) || (e.id ?? '').toLowerCase().includes(needle) || e.data.toLowerCase().includes(needle)) : events).toReversed()
  const pages = Math.max(1, Math.ceil(matched.length / PAGE_SIZE))
  const current = Math.min(page, pages)
  const shown = matched.slice((current - 1) * PAGE_SIZE, current * PAGE_SIZE)
  const done = batch?.done ?? null

  const exportTo = async (format: 'json' | 'sse', copy: boolean) => {
    try {
      const text = await onExport(format)
      if (copy) {
        await host.clipboard.writeText(text)
        toast.success(t('stream.copied'))
        return
      }
      const path = await host.dialog.saveFile(format === 'json' ? 'events.json' : 'events.txt', [format === 'json' ? { name: 'JSON', extensions: ['json'] } : { name: 'Text', extensions: ['txt'] }])
      if (!path) return
      await host.fs.writeText(path, text)
      toast.success(t('common:saved'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const state = run.running ? (
    <span className="flex shrink-0 items-center gap-1.5">
      <Spinner className="size-3" />
      {opened ? t('stream.receiving') : t('stream.connecting')}
    </span>
  ) : done ? (
    <Badge variant={done.reason === 'failed' ? 'danger' : done.reason === 'stopped' ? 'warning' : 'success'}>{t(`stream.end.${done.reason}`)}</Badge>
  ) : null

  return (
    <Panel
      title={
        <Tabs<StreamTab>
          variant="underline"
          value={tab}
          onValueChange={setTab}
          aria-label={t('response.title')}
          items={[
            { value: 'events', label: t('stream.events', { count: batch?.total ?? 0 }) },
            { value: 'headers', label: t('response.headers', { count: opened?.headers.length ?? 0 }) },
          ]}
        />
      }
      actions={
        opened && (
          <>
            <Badge variant={statusTone(opened.status)}>{`${opened.status} ${opened.statusText}`}</Badge>
            <Badge variant="info">{opened.mode === 'sse' ? 'SSE' : t('stream.raw')}</Badge>
            <DropdownMenu
              trigger={
                <Button size="sm" variant="ghost" disabled={events.length === 0}>
                  <ArrowDownToLine />
                  {t('stream.export')}
                </Button>
              }
              items={[
                { key: 'copy-json', label: t('stream.copyJson'), icon: <Copy />, onSelect: () => void exportTo('json', true) },
                { key: 'save-json', label: t('stream.saveJson'), icon: <ArrowDownToLine />, onSelect: () => void exportTo('json', false) },
                { key: 'save-sse', label: opened.mode === 'sse' ? t('stream.saveSse') : t('stream.saveRaw'), icon: <ArrowDownToLine />, onSelect: () => void exportTo('sse', false) },
              ]}
            />
          </>
        )
      }
      footer={
        (run.running || opened) && (
          <>
            {state}
            {batch && (
              <span className="shrink-0 tabular-nums">
                {t('stream.stats', { count: batch.total, size: formatBytes(batch.bytes), seconds: seconds(batch.elapsedMs) })}
              </span>
            )}
            {batch && batch.comments > 0 && <span className="shrink-0">{t('stream.comments', { count: batch.comments })}</span>}
            {batch?.retry != null && <span className="shrink-0">{t('stream.retry', { ms: batch.retry })}</span>}
            {done?.error && <span className="min-w-0 truncate text-danger">{errorMessage(done.error)}</span>}
            {batch?.lastId && (
              <span className="ml-auto flex min-w-0 items-center gap-2">
                <span className="truncate font-mono" title={batch.lastId}>
                  {t('stream.lastId', { id: batch.lastId })}
                </span>
                {done && (
                  <Button size="sm" variant="ghost" onClick={() => onResume(batch.lastId!)}>
                    <RotateCw />
                    {t('stream.resume')}
                  </Button>
                )}
              </span>
            )}
          </>
        )
      }
      bodyClassName="flex min-h-0 flex-col"
    >
      {run.error ? (
        <div className="m-auto max-w-md space-y-1 p-6 text-center">
          <FileWarning className="mx-auto size-6 text-danger" />
          <p className="text-[13px] text-danger">{errorMessage(run.error)}</p>
        </div>
      ) : !opened ? (
        <Empty icon={run.running ? <Spinner /> : <Radio />} title={run.running ? t('stream.connecting') : t('stream.empty')} description={run.running ? undefined : t('stream.emptyHint')} />
      ) : tab === 'headers' ? (
        <table className="w-full text-left text-xs">
          <tbody className="font-mono">
            {opened.headers.map(([name, value], index) => (
              <tr key={`${name}-${index}`} className="border-b border-border align-top">
                <td className="w-1/3 px-3 py-2 text-fg-muted" data-selectable>
                  {name}
                </td>
                <td className="px-3 py-2 break-all text-fg" data-selectable>
                  {value}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : (
        <>
          <div className="flex shrink-0 items-center gap-2 border-b border-border px-3 py-2">
            <Input
              size="sm"
              value={query}
              onChange={(event) => {
                setQuery(event.target.value)
                setPage(1)
              }}
              leading={<Search />}
              placeholder={t('stream.search')}
              wrapperClassName="flex-1"
              aria-label={t('stream.search')}
            />
            {needle && <span className="shrink-0 text-xs text-fg-muted">{t('stream.matched', { count: matched.length })}</span>}
          </div>
          {(batch?.dropped ?? 0) > 0 && (
            <p className="shrink-0 border-b border-border bg-warning-soft/60 px-3 py-2 text-xs text-warning">{t('stream.dropped', { count: batch!.dropped, max: 5000 })}</p>
          )}
          {done?.discarded && <p className="shrink-0 border-b border-border bg-surface-2 px-3 py-2 text-xs text-fg-muted">{t('stream.discarded')}</p>}
          <div className="min-h-0 flex-1 space-y-2 overflow-auto p-3">
            {shown.length === 0 ? (
              <Empty icon={run.running ? <Spinner /> : <Radio />} title={needle ? t('stream.noMatch') : run.running ? t('stream.waitingEvents') : t('stream.noEvents')} />
            ) : (
              shown.map((item) => <EventCard key={item.seq} item={item} />)
            )}
          </div>
          {matched.length > PAGE_SIZE && (
            <div className="shrink-0 border-t border-border px-3 py-2">
              <Pagination
                page={current}
                pageSize={PAGE_SIZE}
                total={matched.length}
                onPageChange={setPage}
                prevLabel={t('stream.prev')}
                nextLabel={t('stream.next')}
                pageLabel={(n) => t('stream.page', { page: n })}
                totalLabel={t('stream.newestFirst', { count: matched.length })}
              />
            </div>
          )}
        </>
      )}
    </Panel>
  )
}
