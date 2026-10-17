import { useEffect, useState } from 'react'
import { Button, CodeEditor, Empty, Input, Pagination, Progress, SegmentedControl, Spinner, cn } from '@toolforge/ui'
import { callPlugin, formatBytes, usePlugin, type useTask } from '@toolforge/plugin-ui-sdk'
import { FileCode2, Search, TriangleAlert } from 'lucide-react'
import { functionLabel } from './format'
import type { FunctionRow, WatInfo } from './types'
import { PAGE_SIZE, usePaged } from './usePaged'

export type WatTask = ReturnType<typeof useTask<WatInfo>>

function FunctionText({ token, index }: { token: number; index: number }) {
  const { t, manifest, errorMessage } = usePlugin()
  const [state, setState] = useState<{ index: number; text: string | null; error: string | null } | null>(null)

  useEffect(() => {
    let alive = true
    callPlugin<{ text: string }>(manifest.id, 'function_wat', { token, index }).then(
      (out) => alive && setState({ index, text: out.text, error: null }),
      (reason) => alive && setState({ index, text: null, error: errorMessage(reason) }),
    )
    return () => {
      alive = false
    }
  }, [manifest.id, errorMessage, token, index])

  if (!state || state.index !== index) {
    return (
      <div className="flex h-full items-center justify-center">
        <Spinner />
      </div>
    )
  }
  if (state.error) return <p className="p-4 text-[13px] text-danger">{state.error}</p>
  return <CodeEditor value={state.text ?? ''} language="text" readOnly aria-label={t('wat.function', { index })} />
}

export function WatView({
  token,
  task,
  autoStart,
  focus,
  onFocus,
  onStart,
}: {
  token: number
  task: WatTask
  autoStart: boolean
  focus: number | null
  onFocus: (index: number) => void
  onStart: () => void
}) {
  const { t, errorMessage } = usePlugin()
  const [mode, setMode] = useState<'outline' | 'functions'>(focus !== null ? 'functions' : 'outline')
  const [query, setQuery] = useState('')
  const [page, setPage] = useState(0)
  const { page: list } = usePaged<FunctionRow>(token, 'functions', query, page)
  const info = task.status === 'succeeded' ? task.result : null

  useEffect(() => {
    if (autoStart && task.status === 'idle') onStart()
  }, [autoStart, task.status]) // eslint-disable-line react-hooks/exhaustive-deps

  if (!info) {
    return (
      <Empty
        icon={<FileCode2 />}
        title={t('wat.title')}
        description={t('wat.hint')}
        action={
          <div className="flex flex-col items-center gap-3">
            {task.running ? (
              <div className="flex w-72 items-center gap-3">
                <Progress value={null} aria-label={t('wat.printing')} />
                <span className="shrink-0 text-xs text-fg-muted">{t('wat.printing')}</span>
                <Button size="sm" onClick={() => void task.cancel()}>
                  {t('open.cancel')}
                </Button>
              </div>
            ) : (
              <Button variant="primary" onClick={onStart}>
                {t('wat.generate')}
              </Button>
            )}
            {task.status === 'failed' && task.error && <p className="text-xs text-danger">{errorMessage(task.error)}</p>}
          </div>
        }
      />
    )
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-3">
        <SegmentedControl
          size="sm"
          value={mode}
          onValueChange={setMode}
          options={[
            { value: 'outline', label: t('wat.outline') },
            { value: 'functions', label: t('wat.functions') },
          ]}
          aria-label={t('wat.title')}
        />
        <span className="text-xs text-fg-muted">
          {t('wat.stats', { size: formatBytes(info.bytes), count: info.functions })}
        </span>
        {(info.truncated || (mode === 'outline' && info.outlineTruncated)) && (
          <span className="flex items-center gap-1 text-xs text-warning">
            <TriangleAlert className="size-3.5" />
            {info.truncated ? t('wat.truncated') : t('wat.outlineTruncated')}
          </span>
        )}
      </div>
      {mode === 'outline' ? (
        <div className="min-h-0 flex-1 overflow-hidden rounded-card border border-border">
          <CodeEditor value={info.outline} language="text" readOnly aria-label={t('wat.outline')} />
        </div>
      ) : (
        <div className="grid min-h-0 flex-1 grid-cols-[280px_1fr] gap-3">
          <div className="flex min-h-0 flex-col gap-2">
            <Input
              value={query}
              onChange={(e) => {
                setQuery(e.target.value)
                setPage(0)
              }}
              placeholder={t('list.search')}
              aria-label={t('list.search')}
              leading={<Search className="size-3.5" />}
            />
            <ul className="min-h-0 flex-1 overflow-auto rounded-card border border-border bg-surface p-1">
              {list?.items.map((f) => (
                <li key={f.index}>
                  <button
                    type="button"
                    onClick={() => onFocus(f.index)}
                    className={cn(
                      'flex w-full items-center gap-2 rounded-control px-2 py-1 text-left text-xs hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none',
                      focus === f.index && 'bg-active',
                    )}
                  >
                    <span className="w-10 shrink-0 font-mono text-fg-subtle">{f.index}</span>
                    <span className="min-w-0 flex-1 truncate font-mono text-fg">{functionLabel(f)}</span>
                    <span className="shrink-0 tabular-nums text-fg-subtle">{formatBytes(f.size)}</span>
                  </button>
                </li>
              ))}
            </ul>
            {list && list.total > PAGE_SIZE && (
              <Pagination
                page={page}
                pageSize={PAGE_SIZE}
                total={list.total}
                onPageChange={setPage}
                prevLabel={t('list.prev')}
                nextLabel={t('list.next')}
                pageLabel={(p) => t('list.page', { page: p })}
              />
            )}
          </div>
          <div className="min-h-0 overflow-hidden rounded-card border border-border">
            {focus === null ? (
              <Empty title={t('wat.pick')} className="h-full" />
            ) : (
              <FunctionText token={token} index={focus} />
            )}
          </div>
        </div>
      )}
    </div>
  )
}
