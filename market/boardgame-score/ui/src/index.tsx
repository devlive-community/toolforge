import { useEffect, useRef, useState } from 'react'
import { Badge, Button, Empty, LogViewer, Panel, SegmentedControl, Select, Spinner, Tooltip, cn, toast } from '@toolforge/ui'
import { host, loadPluginState, savePluginState, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { Copy, Dices, Download, FileJson, Flag, Pencil, Plus, RotateCcw, Square, Trash2, Trophy, Undo2 } from 'lucide-react'
import { Audit, PageBar, RoundTable, Standings } from './Board'
import { GameForm, RoundEntry } from './Forms'
import { emptyState, type Editor, type Exported, type Op, type Result, type State, type View } from './types'

type Confirm = 'finish' | 'reopen' | 'delete' | 'undo'
type Pages = { games: number; rounds: number; events: number }
export default function BoardgameScore() {
  const { t, manifest, errorMessage } = usePlugin()
  const task = useTask<Result>()
  const exporting = useTask<Exported>()
  const [view, setView] = useState<View | null>(null)
  const [revision, setRevision] = useState(0)
  const [busy, setBusy] = useState(true)
  const [exportBusy, setExportBusy] = useState(false)
  const [dirty, setDirty] = useState(false)
  const [status, setStatus] = useState('loading')
  const [saveError, setSaveError] = useState<string | null>(null)
  const [retry, setRetry] = useState(0)
  const [editor, setEditor] = useState<Editor | null>(null)
  const [confirmation, setConfirmation] = useState<Confirm | null>(null)
  const [tab, setTab] = useState<'entry' | 'rounds' | 'audit'>('entry')
  const source = useRef<State | null>(null)
  const pages = useRef<Pages>({ games: 0, rounds: 0, events: 0 })
  const pending = useRef(false)
  const exportPending = useRef(false)
  const sequence = useRef(0)
  const activeRequest = useRef('')
  const handled = useRef('')
  const handledExport = useRef('')
  const closeOnSuccess = useRef(false)
  const mounted = useRef(true)
  const startRef = useRef(task.start)
  const errorRef = useRef(errorMessage)
  const loadRef = useRef<() => void>(() => {})
  const locked = busy || exportBusy
  const navigationLocked = locked || dirty || saveError !== null || editor !== null || confirmation !== null
  const game = view?.active ?? null

  const begin = async (op?: Op, close = false, follow = false, requestedPages = pages.current) => {
    if (pending.current || exportPending.current || !source.current || saveError !== null) return
    pending.current = true
    setBusy(true)
    setStatus(op ? 'saving' : 'loading')
    const requestId = String(++sequence.current)
    activeRequest.current = requestId
    closeOnSuccess.current = close
    const id = await startRef.current(op ? 'apply' : 'inspect', { state: source.current, requestId, op, pages: requestedPages, follow })
    if (id === null) {
      pending.current = false
      setBusy(false)
      setStatus('error')
    }
  }
  const reload = async () => {
    if (pending.current) return
    pending.current = true
    setBusy(true)
    setStatus('loading')
    setSaveError(null)
    try {
      const saved = await loadPluginState<State>(manifest.id, 'scoreboard')
      if (!mounted.current) return
      source.current = saved ?? emptyState()
      pending.current = false
      // Initial load has no pending write to retry.
      const requestId = String(++sequence.current)
      activeRequest.current = requestId
      pending.current = true
      const id = await startRef.current('inspect', { state: source.current, requestId, follow: true, pages: pages.current })
      if (id === null) {
        pending.current = false
        setBusy(false)
        setStatus('error')
      }
    } catch (error) {
      if (mounted.current) {
        pending.current = false
        setBusy(false)
        setStatus('error')
        setSaveError(errorRef.current(error))
      }
    }
  }
  const page = (key: keyof Pages, offset: number) => {
    void begin(undefined, false, false, { ...pages.current, [key]: offset })
  }
  const act = (op: Op, close = false, follow = false) => {
    void begin(op, close, follow)
  }
  const save = async (format: 'csv' | 'json' | 'text') => {
    if (pending.current || exportPending.current || !source.current) return
    exportPending.current = true
    setExportBusy(true)
    exporting.reset()
    try {
      const path = format === 'text' ? null : await host.dialog.saveFile(`boardgame-score.${format}`, [{ name: format.toUpperCase(), extensions: [format] }])
      if (format !== 'text' && !path) {
        exportPending.current = false
        setExportBusy(false)
        return
      }
      const id = await exporting.start('export', { state: source.current, format, language: t('locale'), path })
      if (id === null) {
        exportPending.current = false
        setExportBusy(false)
      }
    } catch (error) {
      exportPending.current = false
      setExportBusy(false)
      toast.error(errorMessage(error))
    }
  }
  useEffect(() => {
    startRef.current = task.start
    errorRef.current = errorMessage
    loadRef.current = () => {
      void reload()
    }
  })
  useEffect(() => {
    mounted.current = true
    loadRef.current()
    return () => {
      mounted.current = false
    }
  }, [manifest.id])
  useEffect(() => {
    const result = task.result
    if (!result || handled.current === result.requestId || activeRequest.current !== result.requestId) return
    handled.current = result.requestId
    Promise.resolve()
      .then(async () => {
        if (source.current?.revision !== result.baseRevision) throw { code: 'boardgame-score.stale' }
        if (result.state) await savePluginState(manifest.id, 'scoreboard', result.state)
        if (!mounted.current) return
        if (result.state) {
          source.current = result.state
          setRevision(result.state.revision)
          setDirty(false)
        }
        pages.current = { games: result.view.games.offset, rounds: result.view.active?.rounds.offset ?? 0, events: result.view.active?.events.offset ?? 0 }
        setView(result.view)
        setStatus(result.state ? 'saved' : 'ready')
        setSaveError(null)
        if (closeOnSuccess.current) {
          setEditor(null)
          setConfirmation(null)
        }
      })
      .catch((error) => {
        if (mounted.current) {
          handled.current = ''
          setSaveError(errorRef.current(error))
          setStatus('error')
        }
      })
      .finally(() => {
        if (mounted.current) {
          pending.current = false
          setBusy(false)
        }
      })
  }, [task.result, manifest.id, retry])
  useEffect(() => {
    if (task.status !== 'failed' && task.status !== 'cancelled') return
    Promise.resolve().then(() => {
      if (mounted.current) {
        pending.current = false
        setBusy(false)
        setStatus(task.status === 'cancelled' ? 'ready' : 'error')
      }
    })
  }, [task.status])
  useEffect(() => {
    const id = exporting.taskId
    if (!id || exporting.running || exporting.status === 'idle' || handledExport.current === id) return
    handledExport.current = id
    Promise.resolve()
      .then(async () => {
        if (exporting.result?.content !== undefined) {
          await host.clipboard.writeText(exporting.result.content)
          toast.success(t('files.copied'))
        } else if (exporting.result?.saved) toast.success(t('files.saved'))
      })
      .catch((error) => toast.error(errorRef.current(error)))
      .finally(() => {
        exportPending.current = false
        if (mounted.current) setExportBusy(false)
      })
  }, [exporting.taskId, exporting.running, exporting.status, exporting.result, t])

  const toolbar = [
    { label: 'actions.rename', icon: Pencil },
    { label: 'actions.copy', icon: Copy },
    { label: 'files.csv', icon: Download },
    { label: 'files.json', icon: FileJson },
    { label: 'files.copy', icon: Trophy },
    { label: 'actions.delete', icon: Trash2 },
  ]
  const toolbarAction = (label: string) => {
    if (label === 'actions.rename') setEditor({ kind: 'rename' })
    else if (label === 'actions.copy') setEditor({ kind: 'copy' })
    else if (label === 'actions.delete') setConfirmation('delete')
    else void save(label === 'files.csv' ? 'csv' : label === 'files.json' ? 'json' : 'text')
  }
  return (
    <div className="@container flex h-full min-h-0 flex-col gap-3">
      <div className="flex shrink-0 flex-wrap items-center gap-2 rounded-card border border-border bg-surface p-3">
        <div className="mr-1 flex size-9 shrink-0 items-center justify-center rounded-control bg-primary-soft text-primary">
          <Dices className="size-5" />
        </div>
        <Select
          className="min-w-36 flex-1"
          aria-label={t('games.title')}
          value={game?.id ?? null}
          onValueChange={(id) => act({ type: 'select', id }, false, true)}
          disabled={navigationLocked || !view?.games.total}
          placeholder={t('games.empty')}
          options={
            view?.gameOptions.map((g) => ({ value: g.id, label: t(g.finished ? 'games.finishedItem' : 'games.activeItem', { name: g.name, n: g.rounds }) })) ??
            []
          }
        />
        {view && <PageBar page={view.games} change={(offset) => page('games', offset)} busy={navigationLocked} />}
        <Button variant="primary" size="sm" disabled={navigationLocked || !view} onClick={() => setEditor({ kind: 'new' })}>
          <Plus />
          {t('games.new')}
        </Button>
      </div>
      <Panel
        className="min-h-0 flex-1"
        icon={<Trophy />}
        title={game?.name ?? t('name')}
        extra={game && <Badge variant={game.finished ? 'success' : 'neutral'}>{t(game.finished ? 'status.finished' : 'status.active')}</Badge>}
        bodyClassName="flex h-full min-h-0 flex-col gap-3 p-3"
        actions={
          game && (
            <div className="flex flex-wrap gap-0.5">
              {toolbar.map(({ label, icon: Icon }) => (
                <Tooltip key={label} content={t(label)}>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    disabled={navigationLocked || (game.finished && label === 'actions.rename')}
                    aria-label={t(label)}
                    onClick={() => toolbarAction(label)}
                  >
                    <Icon />
                  </Button>
                </Tooltip>
              ))}
            </div>
          )
        }
      >
        {task.error && (
          <p role="alert" className="shrink-0 text-xs text-danger">
            {errorMessage(task.error)}
          </p>
        )}
        {exporting.error && (
          <p role="alert" className="shrink-0 text-xs text-danger">
            {errorMessage(exporting.error)}
          </p>
        )}
        {saveError && (
          <div className="shrink-0 space-y-2 rounded-control border border-danger/30 bg-danger/10 p-3">
            <p role="alert" className="text-xs text-danger">
              {t('save.failed')} {saveError}
            </p>
            <Button
              variant="outline"
              size="sm"
              disabled={locked}
              onClick={() => {
                if (task.result && source.current) {
                  pending.current = true
                  setBusy(true)
                  setRetry((n) => n + 1)
                } else void reload()
              }}
            >
              {t('save.retry')}
            </Button>
          </div>
        )}
        {editor ? (
          <GameForm
            key={`${editor.kind}-${editor.round?.number ?? ''}`}
            editor={editor}
            game={game}
            busy={locked || saveError !== null}
            save={(op) => act(op, true, true)}
            cancel={() => setEditor(null)}
          />
        ) : game ? (
          <>
            <div className="flex shrink-0 flex-wrap items-center justify-between gap-2">
              <p className="text-xs text-fg-muted">
                {t(`direction.${game.direction}`)}
                <span className="mx-2 text-fg-subtle">·</span>
                {t('status.rounds', { n: game.confirmed })}
                {game.roundLimit !== null && <span className="ml-2">{t('status.target', { n: game.roundLimit })}</span>}
              </p>
              <div className="flex gap-1">
                <Tooltip content={t('actions.undo')}>
                  <Button
                    size="icon-sm"
                    variant="ghost"
                    aria-label={t('actions.undo')}
                    disabled={navigationLocked || !game.canUndo}
                    onClick={() => setConfirmation('undo')}
                  >
                    <Undo2 />
                  </Button>
                </Tooltip>
                {game.finished ? (
                  <Button size="sm" variant="outline" disabled={navigationLocked} onClick={() => setConfirmation('reopen')}>
                    <RotateCcw />
                    {t('actions.reopen')}
                  </Button>
                ) : (
                  <Button size="sm" variant="outline" disabled={navigationLocked || !game.canFinish} onClick={() => setConfirmation('finish')}>
                    <Flag />
                    {t('actions.finish')}
                  </Button>
                )}
              </div>
            </div>
            {confirmation ? (
              <div className="flex min-h-0 flex-1 flex-col gap-3">
                <div className="shrink-0 rounded-card border border-warning/30 bg-warning/10 p-3">
                  <h2 className="text-sm font-semibold text-fg">{t(`confirm.${confirmation}`)}</h2>
                  <p className="mt-1 text-xs leading-relaxed text-fg-muted">{t(`confirm.${confirmation}Hint`)}</p>
                </div>
                {confirmation === 'finish' && <RoundTable game={game} busy={locked || saveError !== null} page={(offset) => page('rounds', offset)} />}
                <div className="flex gap-2">
                  <Button
                    variant={confirmation === 'delete' ? 'danger' : 'primary'}
                    disabled={locked || saveError !== null}
                    onClick={() => act({ type: confirmation, confirm: true }, true, true)}
                  >
                    {t('common.confirm')}
                  </Button>
                  <Button variant="ghost" disabled={locked || saveError !== null} onClick={() => setConfirmation(null)}>
                    {t('common.cancel')}
                  </Button>
                </div>
              </div>
            ) : (
              <>
                <div className="max-h-[35%] shrink-0 overflow-auto">
                  <Standings game={game} />
                </div>
                <div className={navigationLocked ? 'pointer-events-none shrink-0 opacity-50' : 'shrink-0'}>
                  <SegmentedControl
                    size="sm"
                    value={tab}
                    onValueChange={(value) => {
                      if (!navigationLocked) setTab(value)
                    }}
                    aria-label={t('tabs.label')}
                    options={[
                      { value: 'entry', label: t('tabs.entry') },
                      { value: 'rounds', label: t('tabs.rounds') },
                      { value: 'audit', label: t('tabs.audit') },
                    ]}
                  />
                </div>
                {tab === 'entry' ? (
                  game.canAdd ? (
                    <RoundEntry
                      key={`${game.id}-${revision}`}
                      game={game}
                      busy={locked || saveError !== null}
                      save={(op) => act(op, false, true)}
                      onDirty={setDirty}
                    />
                  ) : (
                    <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-3 overflow-auto rounded-card border border-dashed border-border p-6">
                      <Trophy className="size-8 text-primary" />
                      <h3 className="text-base font-semibold text-fg">{t(game.finished ? 'result.finished' : 'result.ready')}</h3>
                      <p className="max-w-md text-center text-xs leading-relaxed text-fg-muted">
                        {t(game.finished ? 'result.finishedHint' : 'result.readyHint')}
                      </p>
                      <Button variant="outline" disabled={navigationLocked} onClick={() => setTab('rounds')}>
                        {t('tabs.rounds')}
                      </Button>
                    </div>
                  )
                ) : tab === 'rounds' ? (
                  <RoundTable
                    game={game}
                    busy={navigationLocked}
                    page={(offset) => page('rounds', offset)}
                    edit={(round) => setEditor({ kind: 'edit', round })}
                  />
                ) : (
                  <Audit game={game} busy={navigationLocked} page={(offset) => page('events', offset)} />
                )}
              </>
            )}
          </>
        ) : (
          <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-4">
            <Empty icon={<Dices />} title={t('games.welcome')} description={t('games.hint')} />
            <Button variant="primary" disabled={navigationLocked || !view} onClick={() => setEditor({ kind: 'new' })}>
              <Plus />
              {t('games.new')}
            </Button>
          </div>
        )}
        <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-t border-border pt-2">
          <p role="status" className={cn('flex items-center gap-1.5 text-[11px]', status === 'error' ? 'text-danger' : 'text-fg-subtle')}>
            {locked && <Spinner className="size-3" />}
            {t(`save.${status}`)}
          </p>
          {task.running && (
            <Button
              size="sm"
              variant="ghost"
              onClick={() => {
                void task.cancel().catch((error) => toast.error(errorMessage(error)))
              }}
            >
              <Square />
              {t('common.cancel')}
            </Button>
          )}
        </div>
        {(task.logs.length > 0 || exporting.logs.length > 0) && (
          <details className="shrink-0 rounded-control border border-border">
            <summary className="cursor-pointer px-2 py-1 text-[11px] text-fg-subtle">{t('logs.title')}</summary>
            <LogViewer entries={exportBusy ? exporting.logs : task.logs} className="h-20" emptyText={t('logs.empty')} jumpLabel={t('logs.latest')} />
          </details>
        )}
      </Panel>
    </div>
  )
}
