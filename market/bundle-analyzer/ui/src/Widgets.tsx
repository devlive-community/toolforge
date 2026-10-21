import { useEffect, useRef } from 'react'
import { usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { Badge, Button, Pagination, Spinner } from '@toolforge/ui'
import { Square } from 'lucide-react'
import type { Query } from './types'
export const PAGE = 25
export function useRequest<T>(fn: string, args: object) {
  const task = useTask<T>()
  const initial = useRef(args)
  const { start, cancel } = task
  useEffect(() => {
    void start(fn, initial.current)
    return () => {
      void cancel()
    }
  }, [fn, start, cancel])
  return task
}
export function Busy({ task }: { task: { error: unknown; running: boolean; cancel: () => Promise<void> } }) {
  const { t, errorMessage } = usePlugin()
  return (
    <>
      {task.error != null && (
        <p role="alert" className="break-words text-xs text-danger">
          {errorMessage(task.error)}
        </p>
      )}
      {task.running && (
        <div className="flex items-center gap-2 text-xs text-fg-muted">
          <Spinner className="size-3" />
          {t('loading')}
          <Button size="sm" onClick={() => void task.cancel().catch(() => {})}>
            <Square />
            {t('cancel')}
          </Button>
        </div>
      )}
    </>
  )
}
export function Bytes({ value }: { value: number | null | undefined }) {
  const { t } = usePlugin()
  return <span className="font-mono tabular-nums">{value == null ? t('missing') : t('bytes', { value: value.toLocaleString() })}</span>
}
export function Delta({ value }: { value: number }) {
  const { t } = usePlugin()
  return (
    <span className={value > 0 ? 'font-mono text-danger' : value < 0 ? 'font-mono text-success' : 'font-mono text-fg-muted'}>
      {t('delta', { value: (value > 0 ? '+' : '') + value.toLocaleString() })}
    </span>
  )
}
export function Pager({ page, total, setPage }: { page: number; total: number; setPage: (n: number) => void }) {
  const { t } = usePlugin()
  return (
    <Pagination
      page={page}
      pageSize={PAGE}
      total={total}
      onPageChange={setPage}
      prevLabel={t('page.previous')}
      nextLabel={t('page.next')}
      pageLabel={(page) => t('page.label', { page })}
    />
  )
}
export function Kind({ external, resolved }: { external: boolean; resolved: boolean }) {
  const { t } = usePlugin()
  return (
    <Badge variant={external ? 'neutral' : resolved ? 'success' : 'warning'}>
      {t(external ? 'imports.external' : resolved ? 'imports.internal' : 'imports.unresolved')}
    </Badge>
  )
}
export function key(q: Query) {
  return JSON.stringify(q)
}
