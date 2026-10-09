import { useState } from 'react'
import { Badge, Button, Empty, Input, Modal, Panel, Switch, Tabs, Tooltip, cn, toast } from '@toolforge/ui'
import { host, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { FolderOpen, MousePointerClick, RefreshCw, RotateCcw, Search, ShieldAlert } from 'lucide-react'

type Scope = 'files' | 'folders' | 'background' | 'desktop' | 'drives'
type Hive = 'user' | 'machine'
interface Entry {
  hive: Hive
  path: string
  scope: Scope
  kind: 'command' | 'handler'
  key: string
  name: string
  target: string | null
  clsid: string | null
  enabled: boolean
  extended: boolean
}
interface Classic {
  supported: boolean
  enabled: boolean
}

const SCOPES: Scope[] = ['files', 'folders', 'background', 'desktop', 'drives']

function ContextMenu() {
  const { t, call, errorMessage } = usePlugin()
  const [scope, setScope] = useState<Scope>('files')
  const [query, setQuery] = useState('')
  const [version, setVersion] = useState(0)
  const [busy, setBusy] = useState<string | null>(null)
  const [confirmRestart, setConfirmRestart] = useState(false)
  const [lastUndo, setLastUndo] = useState<string | null>(null)
  const list = useDebouncedCall<Entry[]>('list', { scope }, [scope, version])
  const classic = useDebouncedCall<Classic>('classic', {}, [version])

  const reload = () => setVersion((v) => v + 1)
  const showUndo = (path: string) => {
    setLastUndo(path)
    toast.success(t('changed'))
  }

  const toggle = async (entry: Entry, enabled: boolean) => {
    setBusy(entry.path + entry.clsid)
    try {
      const { undo } = await call<{ undo: string }>('set', { hive: entry.hive, path: entry.path, kind: entry.kind, clsid: entry.clsid, enabled, name: entry.name })
      showUndo(undo)
      reload()
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setBusy(null)
    }
  }
  const toggleClassic = async (enabled: boolean) => {
    try {
      const { undo } = await call<{ undo: string }>('set_classic', { enabled })
      showUndo(undo)
      reload()
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const restart = async () => {
    setConfirmRestart(false)
    try {
      await call('restart_explorer', {})
      toast.success(t('explorer.restarted'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const openUndoFolder = async () => {
    try {
      const { path } = await call<{ path: string }>('undo_dir', {})
      await host.revealPath(path)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const needle = query.trim().toLowerCase()
  const entries = (list.result ?? []).filter(
    (e) => !needle || [e.name, e.key, e.target ?? '', e.clsid ?? ''].some((s) => s.toLowerCase().includes(needle)),
  )

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      {classic.result?.supported && (
        <div className="flex shrink-0 items-center gap-3 rounded-card border border-border bg-surface px-4 py-3">
          <MousePointerClick className="size-5 text-fg-muted" />
          <div className="min-w-0 flex-1">
            <p className="text-[13px] font-medium text-fg">{t('classic.title')}</p>
            <p className="text-[11px] text-fg-subtle">{t('classic.hint')}</p>
          </div>
          <Switch checked={classic.result.enabled} onCheckedChange={toggleClassic} aria-label={t('classic.title')} />
        </div>
      )}

      <Panel
        icon={<MousePointerClick />}
        title={t('entries.title')}
        extra={list.result && <Badge>{entries.length}</Badge>}
        actions={
          <>
            <Input size="sm" value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t('entries.search')} leading={<Search />} wrapperClassName="w-56" aria-label={t('entries.search')} />
            <Tooltip content={t('entries.refresh')}>
              <Button size="icon-sm" variant="ghost" aria-label={t('entries.refresh')} onClick={reload}>
                <RefreshCw />
              </Button>
            </Tooltip>
          </>
        }
        footer={
          <div className="flex w-full items-center gap-2">
            {lastUndo ? (
              <span className="flex min-w-0 flex-1 items-center gap-1.5">
                <span className="truncate" title={lastUndo}>
                  {t('undo.saved', { name: lastUndo.split(/[\\/]/).pop() })}
                </span>
                <Button size="sm" variant="ghost" onClick={() => host.revealPath(lastUndo).catch(() => {})}>
                  {t('undo.show')}
                </Button>
              </span>
            ) : (
              <span className="min-w-0 flex-1 truncate">{t('entries.footer')}</span>
            )}
            <Button size="sm" variant="ghost" onClick={openUndoFolder}>
              <FolderOpen />
              {t('undo.folder')}
            </Button>
            <Button size="sm" onClick={() => setConfirmRestart(true)}>
              <RotateCcw />
              {t('explorer.restart')}
            </Button>
          </div>
        }
        className="min-h-0 flex-1"
        bodyClassName="flex min-h-0 flex-col p-0"
      >
        <div className="shrink-0 border-b border-border px-3 py-2">
          <Tabs<Scope> value={scope} onValueChange={setScope} aria-label={t('entries.scope')} items={SCOPES.map((value) => ({ value, label: t(`scopes.${value}`) }))} />
        </div>
        <div className={cn('min-h-0 flex-1 overflow-auto p-2', list.pending && 'opacity-60')}>
          {list.error ? (
            <p className="p-2 text-[13px] text-danger">{errorMessage(list.error)}</p>
          ) : entries.length === 0 ? (
            <Empty icon={<MousePointerClick />} title={t(needle ? 'entries.noMatch' : 'entries.empty')} />
          ) : (
            <ul className="space-y-0.5">
              {entries.map((entry) => (
                <li key={`${entry.hive}:${entry.path}:${entry.clsid}`} className="flex items-center gap-3 rounded-control px-2.5 py-2 hover:bg-hover">
                  <Switch
                    checked={entry.enabled}
                    disabled={busy === entry.path + entry.clsid}
                    onCheckedChange={(on) => void toggle(entry, on)}
                    aria-label={entry.name}
                  />
                  <div className="min-w-0 flex-1">
                    <p className="flex items-center gap-1.5 text-[13px]">
                      <span className={cn('truncate', entry.enabled ? 'text-fg' : 'text-fg-subtle line-through')}>{entry.name}</span>
                      <Badge variant={entry.kind === 'command' ? 'neutral' : 'info'}>{t(`kinds.${entry.kind}`)}</Badge>
                      {entry.extended && <Badge>{t('entries.extended')}</Badge>}
                      {(entry.hive === 'machine' || entry.kind === 'handler') && (
                        <Tooltip content={t('entries.adminHint')}>
                          <span className="inline-flex">
                            <Badge variant="warning">
                              <ShieldAlert className="size-3" />
                              {t('entries.machine')}
                            </Badge>
                          </span>
                        </Tooltip>
                      )}
                    </p>
                    <p className="truncate font-mono text-[11px] text-fg-subtle" title={entry.target ?? entry.clsid ?? entry.key}>
                      {entry.target ?? entry.clsid ?? entry.key}
                    </p>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </div>
      </Panel>

      <Modal
        open={confirmRestart}
        onOpenChange={setConfirmRestart}
        title={t('explorer.confirmTitle')}
        description={t('explorer.confirm')}
        footer={
          <>
            <Button onClick={() => setConfirmRestart(false)}>{t('common:cancel')}</Button>
            <Button variant="primary" onClick={restart}>
              {t('explorer.restart')}
            </Button>
          </>
        }
      />
    </div>
  )
}

export default ContextMenu
