import { useEffect, useRef, useState } from 'react'
import { loadPluginState, savePluginState, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Button, Empty, Panel, Spinner, Tooltip } from '@toolforge/ui'
import { BookOpen, Plus, RefreshCw, Save, Trash2 } from 'lucide-react'
import { Pages } from './Editors'
import type { Book, BookResult, Tariff } from './types'
export function Library({
  tariff,
  apply,
  disabled,
  onBusy,
}: {
  tariff: Tariff
  apply: (tariff: Tariff, append: boolean) => void
  disabled: boolean
  onBusy: (busy: boolean) => void
}) {
  const { t, manifest, call, errorMessage } = usePlugin()
  const [data, setData] = useState<BookResult | null>(null)
  const [busy, setBusy] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [writeFailed, setWriteFailed] = useState(false)
  useEffect(() => {
    onBusy(busy || writeFailed)
    return () => onBusy(false)
  }, [busy, writeFailed, onBusy])
  const [confirm, setConfirm] = useState<string | null>(null)
  const source = useRef<Book | null>(null)
  const candidate = useRef<BookResult | null>(null)
  const pending = useRef(false)
  const alive = useRef(true)
  const api = useRef({ call, errorMessage })
  useEffect(() => {
    api.current = { call, errorMessage }
  })
  const load = async () => {
    if (pending.current) return
    pending.current = true
    setBusy(true)
    try {
      const state = (await loadPluginState<Book>(manifest.id, 'tariffs')) ?? { schemaVersion: 1, revision: 0, tariffs: [] }
      const result = await api.current.call<BookResult>('book', { state })
      if (alive.current) {
        source.current = result.state
        setData(result)
        setError(null)
      }
    } catch (e) {
      if (alive.current) setError(api.current.errorMessage(e))
    } finally {
      pending.current = false
      if (alive.current) setBusy(false)
    }
  }
  const save = async (result: BookResult) => {
    candidate.current = result
    await savePluginState(manifest.id, 'tariffs', result.state)
    if (!alive.current) return
    source.current = result.state
    candidate.current = null
    setData(result)
    setConfirm(null)
    setError(null)
    setWriteFailed(false)
  }
  const update = async (tariff?: Tariff, remove?: string, offset = data?.page.offset ?? 0) => {
    if (pending.current || candidate.current || !source.current) return
    pending.current = true
    setBusy(true)
    try {
      const result = await call<BookResult>('book', { state: source.current, tariff, remove, offset, limit: 5 })
      if (tariff || remove) await save(result)
      else if (alive.current) setData(result)
    } catch (e) {
      if (alive.current) {
        setError(errorMessage(e))
        setWriteFailed(candidate.current !== null)
      }
    } finally {
      pending.current = false
      if (alive.current) setBusy(false)
    }
  }
  const retry = async () => {
    if (pending.current || !candidate.current) return
    pending.current = true
    setBusy(true)
    try {
      await save(candidate.current)
    } catch (e) {
      if (alive.current) setError(errorMessage(e))
    } finally {
      pending.current = false
      if (alive.current) setBusy(false)
    }
  }
  const loader = useRef(load)
  useEffect(() => {
    loader.current = load
  })
  useEffect(() => {
    alive.current = true
    void loader.current()
    return () => {
      alive.current = false
    }
  }, [manifest.id])
  const locked = busy || disabled || writeFailed
  return (
    <Panel title={t('library.title')} icon={<BookOpen />} bodyClassName="space-y-3 p-3">
      <p className="text-[11px] leading-relaxed text-fg-subtle">{t('library.hint')}</p>
      <Button size="sm" disabled={locked || !data} onClick={() => void update(tariff)}>
        <Save />
        {t('library.save')}
      </Button>
      {error && (
        <div className="space-y-2 rounded-control bg-danger/5 p-2 text-xs text-danger" role="alert">
          <p>{error}</p>
          <Button size="sm" disabled={busy} onClick={() => void (candidate.current ? retry() : load())}>
            <RefreshCw />
            {t(writeFailed ? 'library.retry' : 'library.reload')}
          </Button>
          {writeFailed && <p>{t('library.failed')}</p>}
        </div>
      )}
      {busy ? (
        <Spinner className="size-4" />
      ) : (
        data && (
          <div className="space-y-2">
            {data.page.items.map((saved) => (
              <div key={saved.id} className="space-y-2 rounded-control border border-border p-2">
                <div className="flex items-center gap-2">
                  <span className="min-w-0 flex-1 truncate text-xs font-medium text-fg">{saved.name}</span>
                  <span className="text-[10px] text-fg-subtle">{t(`tariff.${saved.pricing.kind}`)}</span>
                  <Tooltip content={t('library.delete')}>
                    <Button size="icon-sm" variant="ghost" disabled={locked} aria-label={t('library.delete')} onClick={() => setConfirm(saved.id)}>
                      <Trash2 />
                    </Button>
                  </Tooltip>
                </div>
                {confirm === saved.id ? (
                  <div className="flex flex-wrap gap-2">
                    <Button size="sm" disabled={locked} onClick={() => void update(undefined, saved.id)}>
                      {t('library.confirmDelete')}
                    </Button>
                    <Button size="sm" variant="ghost" disabled={locked} onClick={() => setConfirm(null)}>
                      {t('common.cancel')}
                    </Button>
                  </div>
                ) : (
                  <div className="flex gap-2">
                    <Button size="sm" disabled={locked} onClick={() => apply(saved, false)}>
                      {t('library.use')}
                    </Button>
                    <Button size="sm" variant="ghost" disabled={locked} onClick={() => apply(saved, true)}>
                      <Plus />
                      {t('library.compare')}
                    </Button>
                  </div>
                )}
              </div>
            ))}
            {data.page.total === 0 && <Empty title={t('library.empty')} />}
            <div className={locked ? 'pointer-events-none opacity-50' : ''}>
              <Pages data={data.page} change={(offset) => void update(undefined, undefined, offset)} />
            </div>
          </div>
        )
      )}
    </Panel>
  )
}
