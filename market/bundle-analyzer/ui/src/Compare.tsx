import { useState } from 'react'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { Badge, Button, Input } from '@toolforge/ui'
import { ArrowLeft, Link2, Plus, X } from 'lucide-react'
import { Busy, Bytes, Delta, Pager, PAGE, key, useRequest } from './Widgets'
import type { Change, ModuleChange, Page, Pair, Query } from './types'

export function ChangePage({ q, page, setPage, select }: { q: Query; page: number; setPage: (n: number) => void; select: (row: Change) => void }) {
  const { t } = usePlugin()
  const task = useRequest<Page<Change>>('query', { ...q, offset: (page - 1) * PAGE, limit: PAGE })
  return (
    <>
      <Busy task={task} />
      <div className="min-h-0 flex-1 space-y-2 overflow-auto">
        {task.result?.items.map((row, i) => (
          <button
            key={i}
            type="button"
            onClick={() => select(row)}
            className="w-full space-y-2 rounded-control border border-border p-3 text-left hover:bg-hover"
          >
            <div className="flex items-center justify-between gap-2">
              <Badge variant={row.mode === 'exact' || row.mode === 'manual' ? 'neutral' : 'warning'}>{t(`compare.${row.mode}`)}</Badge>
              <Delta value={row.delta} />
            </div>
            <p className="break-words font-mono text-xs text-fg">{row.before ?? t('missing')}</p>
            <p className="break-words font-mono text-xs text-fg-muted">{row.after ?? t('missing')}</p>
            <div className="flex gap-3 text-[11px] text-fg-muted">
              <Bytes value={row.beforeBytes} />
              <span>{t('compare.to')}</span>
              <Bytes value={row.afterBytes} />
            </div>
          </button>
        ))}
        {task.result?.total === 0 && <p className="text-xs text-fg-muted">{t('emptyRows')}</p>}
      </div>
      <Pager page={page} total={task.result?.total ?? 0} setPage={setPage} />
    </>
  )
}
function ModuleChanges({ q, page, setPage }: { q: Query; page: number; setPage: (n: number) => void }) {
  const { t } = usePlugin()
  const task = useRequest<Page<ModuleChange>>('query', { ...q, offset: (page - 1) * PAGE, limit: PAGE })
  return (
    <>
      <Busy task={task} />
      <div className="min-h-0 flex-1 space-y-2 overflow-auto">
        {task.result?.items.map((row) => (
          <div key={row.path} className="space-y-1 rounded-control border border-border p-3">
            <div className="flex items-center justify-between gap-3">
              <code className="min-w-0 break-words text-xs text-fg">{row.path}</code>
              <Delta value={row.delta} />
            </div>
            <p className="flex gap-3 text-[11px] text-fg-muted">
              <Bytes value={row.beforeBytes} />
              <span>{t('compare.to')}</span>
              <Bytes value={row.afterBytes} />
            </p>
          </div>
        ))}
        {task.result?.total === 0 && <p className="text-xs text-fg-muted">{t('emptyRows')}</p>}
      </div>
      <Pager page={page} total={task.result?.total ?? 0} setPage={setPage} />
    </>
  )
}
export function Compare({
  q,
  pairs,
  setPairs,
  selected,
  select,
  search,
  setSearch,
}: {
  q: Query
  pairs: Pair[]
  setPairs: (pairs: Pair[]) => void
  selected: Change | null
  select: (c: Change | null) => void
  search: string
  setSearch: (v: string) => void
}) {
  const { t } = usePlugin()
  const [before, setBefore] = useState('')
  const [after, setAfter] = useState('')
  const [page, setPage] = useState(1)
  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <p className="shrink-0 text-[11px] leading-relaxed text-fg-muted">{t('compare.hint')}</p>
      <div className="grid shrink-0 grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto] gap-2">
        <Input
          size="sm"
          value={before}
          maxLength={2048}
          onChange={(e) => setBefore(e.target.value)}
          placeholder={t('compare.beforePath')}
          aria-label={t('compare.beforePath')}
        />
        <Input
          size="sm"
          value={after}
          maxLength={2048}
          onChange={(e) => setAfter(e.target.value)}
          placeholder={t('compare.afterPath')}
          aria-label={t('compare.afterPath')}
        />
        <Button
          size="sm"
          disabled={!before || !after}
          onClick={() => {
            setPairs([...pairs, { before, after }])
            select(null)
            setPage(1)
            setBefore('')
            setAfter('')
          }}
        >
          <Plus />
          {t('compare.pair')}
        </Button>
      </div>
      {pairs.length > 0 && (
        <details className="shrink-0 rounded-control border border-border">
          <summary className="cursor-pointer p-2 text-xs text-fg-muted">{t('compare.pairs', { count: pairs.length })}</summary>
          <PairList
            key={JSON.stringify(pairs)}
            q={{ ...q, kind: 'pairings' }}
            remove={(index) => {
              setPairs(pairs.filter((_, i) => i !== index))
              select(null)
              setPage(1)
            }}
          />
        </details>
      )}
      {selected && (
        <div className="shrink-0 space-y-2 rounded-control border border-border p-2">
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              select(null)
              setPage(1)
            }}
          >
            <ArrowLeft />
            {t('compare.back')}
          </Button>
          <p className="break-words font-mono text-xs text-fg">
            {selected.before ?? t('missing')} {t('compare.to')} {selected.after ?? t('missing')}
          </p>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              if (selected.before) setBefore(selected.before)
              if (selected.after) setAfter(selected.after)
            }}
          >
            <Link2 />
            {t('compare.usePaths')}
          </Button>
        </div>
      )}
      <Input
        size="sm"
        value={search}
        maxLength={512}
        onChange={(e) => {
          setSearch(e.target.value)
          setPage(1)
        }}
        aria-label={t('compare.search')}
        placeholder={t('compare.search')}
      />
      {selected ? (
        <ModuleChanges key={`${key(q)}:${page}`} q={q} page={page} setPage={setPage} />
      ) : (
        <ChangePage
          key={`${key(q)}:${page}`}
          q={q}
          page={page}
          setPage={setPage}
          select={(row) => {
            select(row)
            setSearch('')
            setPage(1)
            if (row.before) setBefore(row.before)
            if (row.after) setAfter(row.after)
          }}
        />
      )}
    </div>
  )
}
function PairList({ q, remove }: { q: Query; remove: (i: number) => void }) {
  const [page, setPage] = useState(1)
  return <PairPage key={`${key(q)}:${page}`} q={q} page={page} setPage={setPage} remove={remove} />
}
function PairPage({ q, page, setPage, remove }: { q: Query; page: number; setPage: (n: number) => void; remove: (i: number) => void }) {
  const { t } = usePlugin()
  const task = useRequest<Page<Pair & { id: number }>>('query', { ...q, offset: (page - 1) * PAGE, limit: PAGE })
  return (
    <div className="max-h-44 space-y-2 overflow-auto p-2">
      <Busy task={task} />
      {task.result?.items.map((pair) => (
        <div key={pair.id} className="flex items-center gap-2 text-[11px]">
          <span className="min-w-0 flex-1 break-words font-mono text-fg-muted">
            {pair.before} {t('compare.to')} {pair.after}
          </span>
          <Button size="sm" variant="ghost" onClick={() => remove(pair.id)}>
            <X />
            {t('compare.remove')}
          </Button>
        </div>
      ))}
      <Pager page={page} total={task.result?.total ?? 0} setPage={setPage} />
    </div>
  )
}
