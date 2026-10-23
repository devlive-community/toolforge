import { useEffect, useRef, useState } from 'react'
import { Badge, Button, Empty, Input, LogViewer, Panel, SegmentedControl, Select, Spinner, Tooltip, cn, toast } from '@toolforge/ui'
import { host, loadPluginState, savePluginState, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { Archive, Download, FileJson, Leaf, Pencil, Plus, RefreshCw, Square, Trash2, Upload } from 'lucide-react'
import { Form } from './Editor'
import { CareList, Gallery, PageBar, Timeline } from './Journal'
import { emptyGarden, emptyQuery, type Editor, type Garden, type Op, type Photo, type Query, type Result, type Section, type View } from './types'
interface Confirmation {
  kind: 'deletePlant' | 'removePhoto' | 'void' | 'archive' | 'restore'
  op?: Op
  path?: string
}
export default function PlantCare() {
  const { t, manifest, call, errorMessage } = usePlugin()
  const task = useTask<Result>()
  const exporting = useTask<{ content?: string; saved?: boolean }>()
  const photoTask = useTask<{ photos: Photo[] }>()
  const [view, setView] = useState<View | null>(null)
  const [busy, setBusy] = useState(true)
  const [extraBusy, setExtraBusy] = useState(false)
  const [status, setStatus] = useState('loading')
  const [saveError, setSaveError] = useState<string | null>(null)
  const [retry, setRetry] = useState(0)
  const [editor, setEditor] = useState<Editor | null>(null)
  const [confirmation, setConfirmation] = useState<Confirmation | null>(null)
  const [previewDue, setPreviewDue] = useState<string | null>(null)
  const [query, setQuery] = useState<Query>(emptyQuery)
  const [search, setSearch] = useState('')
  const [pane, setPane] = useState<'plants' | 'journal'>('journal')
  const [picked, setPicked] = useState<string[]>([])
  const [photoView, setPhotoView] = useState<Photo[] | null>(null)
  const source = useRef<Garden | null>(null)
  const session = useRef('')
  const sequence = useRef(0)
  const pending = useRef(false)
  const extraPending = useRef(false)
  const activeRequest = useRef('')
  const activeQuery = useRef<Query>(emptyQuery)
  const handled = useRef('')
  const handledExport = useRef('')
  const handledPhotos = useRef('')
  const closeOnSuccess = useRef(false)
  const lastDay = useRef('')
  const mounted = useRef(true)
  const callRef = useRef(call)
  const startRef = useRef(task.start)
  const errorRef = useRef(errorMessage)
  const loadRef = useRef<() => void>(() => {})
  const refreshRef = useRef<() => void>(() => {})
  const locked = busy || extraBusy
  const navigationLocked = locked || saveError !== null || editor !== null || confirmation !== null || photoView !== null
  const begin = async (fn: 'inspect' | 'apply' | 'preview' | 'restore' = 'inspect', op?: Op, close = false, nextQuery = activeQuery.current, path?: string) => {
    if (pending.current || extraPending.current || !source.current || saveError !== null) return
    pending.current = true
    setBusy(true)
    setStatus(fn === 'inspect' ? 'loading' : fn === 'preview' ? 'previewing' : 'saving')
    if (fn === 'preview') setPreviewDue(null)
    const requestId = `${session.current}-${++sequence.current}`
    activeRequest.current = requestId
    closeOnSuccess.current = close
    activeQuery.current = nextQuery
    try {
      const clock = await callRef.current<{ today: string }>('today')
      const id = await startRef.current(fn, { state: source.current, today: clock.today, requestId, key: requestId, query: nextQuery, op, path })
      if (id === null) {
        pending.current = false
        setBusy(false)
        setStatus('error')
      }
    } catch (error) {
      pending.current = false
      setBusy(false)
      setStatus('error')
      toast.error(errorRef.current(error))
    }
  }
  const reload = async () => {
    if (pending.current) return
    pending.current = true
    setBusy(true)
    setSaveError(null)
    try {
      const [saved, boot] = await Promise.all([
        loadPluginState<Garden>(manifest.id, 'garden'),
        callRef.current<{ session: string; today: string }>('bootstrap'),
      ])
      if (!mounted.current) return
      source.current = saved ?? emptyGarden()
      session.current = boot.session
      const requestId = `${boot.session}-${++sequence.current}`
      activeRequest.current = requestId
      const id = await startRef.current('inspect', { state: source.current, today: boot.today, requestId, query: activeQuery.current })
      if (id === null) {
        pending.current = false
        setBusy(false)
        setStatus('error')
      }
    } catch (error) {
      if (mounted.current) {
        pending.current = false
        setBusy(false)
        setSaveError(errorRef.current(error))
        setStatus('error')
      }
    }
  }
  const action = (op: Op, close = false) => {
    void begin('apply', op, close)
  }
  const changePage = (offset: number, plants = false) => {
    void begin('inspect', undefined, false, { ...activeQuery.current, [plants ? 'plantsOffset' : 'offset']: offset })
  }
  const changeSection = (section: Section) => {
    setPane('journal')
    void begin('inspect', undefined, false, { ...activeQuery.current, section, offset: 0 })
  }
  const open = (next: Editor) => {
    if (!navigationLocked) {
      setPreviewDue(null)
      setEditor(next)
      setPane('journal')
    }
  }
  const confirm = (op: Op) => {
    if (!navigationLocked) setConfirmation({ kind: op.type as Confirmation['kind'], op })
  }
  const choosePhoto = async () => {
    if (!view?.selected) return
    const plantId = view.selected.id
    try {
      const path = await host.dialog.openFile([{ name: t('photos.files'), extensions: ['png', 'jpg', 'jpeg', 'webp'] }])
      if (path) {
        setEditor({ kind: 'photo', plantId, path })
        setPane('journal')
      }
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const chooseRestore = async () => {
    try {
      const path = await host.dialog.openFile([{ name: t('files.json'), extensions: ['json'] }])
      if (path) setConfirmation({ kind: 'restore', path })
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const exportFile = async (format: 'json' | 'markdown') => {
    if (pending.current || extraPending.current || !source.current || !view) return
    extraPending.current = true
    setExtraBusy(true)
    exporting.reset()
    try {
      const ext = format === 'json' ? 'json' : 'md'
      const path = await host.dialog.saveFile(`plant-care-journal.${ext}`, [{ name: ext.toUpperCase(), extensions: [ext] }])
      if (!path) {
        extraPending.current = false
        setExtraBusy(false)
        return
      }
      const id = await exporting.start('export', { state: source.current, today: view.today, requestId: 'export', format, language: t('locale'), path })
      if (id === null) {
        extraPending.current = false
        setExtraBusy(false)
      }
    } catch (error) {
      extraPending.current = false
      setExtraBusy(false)
      toast.error(errorMessage(error))
    }
  }
  const comparePhotos = async () => {
    if (pending.current || extraPending.current || !source.current || !view || picked.length === 0) return
    extraPending.current = true
    setExtraBusy(true)
    const id = await photoTask.start('photos', { state: source.current, today: view.today, requestId: 'photos', ids: picked })
    if (id === null) {
      extraPending.current = false
      setExtraBusy(false)
    }
  }
  useEffect(() => {
    callRef.current = call
    startRef.current = task.start
    errorRef.current = errorMessage
    loadRef.current = () => {
      void reload()
    }
    lastDay.current = view?.today ?? ''
    refreshRef.current = () => {
      if (!navigationLocked)
        void callRef
          .current<{ today: string }>('today')
          .then((clock) => {
            if (clock.today !== lastDay.current) void begin()
          })
          .catch(() => {})
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
    const timer = setInterval(() => refreshRef.current(), 60000)
    return () => clearInterval(timer)
  }, [])
  useEffect(() => {
    const result = task.result
    if (!result || handled.current === result.requestId || activeRequest.current !== result.requestId) return
    handled.current = result.requestId
    Promise.resolve()
      .then(async () => {
        if (source.current?.revision !== result.baseRevision) throw { code: 'plant-care.stale' }
        if (result.preview) {
          setPreviewDue(result.previewDue)
          setStatus('ready')
          return
        }
        if (result.state) await savePluginState(manifest.id, 'garden', result.state)
        if (!mounted.current) return
        if (result.state) {
          source.current = result.state
          setPicked([])
        }
        setView(result.view)
        setQuery(activeQuery.current)
        setStatus(result.state ? 'saved' : 'ready')
        setSaveError(null)
        if (closeOnSuccess.current) {
          setEditor(null)
          setConfirmation(null)
          setPreviewDue(null)
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
      .then(() => {
        if (exporting.result?.saved) toast.success(t('files.saved'))
      })
      .finally(() => {
        extraPending.current = false
        if (mounted.current) setExtraBusy(false)
      })
  }, [exporting.taskId, exporting.running, exporting.status, exporting.result, t])
  useEffect(() => {
    const id = photoTask.taskId
    if (!id || photoTask.running || photoTask.status === 'idle' || handledPhotos.current === id) return
    handledPhotos.current = id
    Promise.resolve()
      .then(() => {
        if (photoTask.result && mounted.current) setPhotoView(photoTask.result.photos)
      })
      .finally(() => {
        extraPending.current = false
        if (mounted.current) setExtraBusy(false)
      })
  }, [photoTask.taskId, photoTask.running, photoTask.status, photoTask.result])
  const plant = view?.selected
  return (
    <div className="@container flex h-full min-h-0 flex-col gap-3">
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 rounded-card border border-border bg-surface p-3">
        <div className="flex items-center gap-3">
          <div className="rounded-control bg-primary-soft p-2 text-primary">
            <Leaf className="size-5" />
          </div>
          <div>
            <h2 className="text-sm font-semibold text-fg">{t('name')}</h2>
            <p className="mt-0.5 text-[11px] text-fg-muted">
              {t('overview', { today: view?.today ?? '', overdue: view?.counts.overdue ?? 0, due: view?.counts.today ?? 0 })}
            </p>
          </div>
          <Tooltip content={t('actions.refresh')}>
            <Button
              size="icon-sm"
              variant="ghost"
              disabled={navigationLocked || !view}
              aria-label={t('actions.refresh')}
              onClick={() => {
                void begin()
              }}
            >
              <RefreshCw />
            </Button>
          </Tooltip>
        </div>
        <div className="flex items-center gap-1">
          <Tooltip content={t('files.json')}>
            <Button size="icon-sm" variant="ghost" disabled={navigationLocked || !view} aria-label={t('files.json')} onClick={() => void exportFile('json')}>
              <FileJson />
            </Button>
          </Tooltip>
          <Tooltip content={t('files.markdown')}>
            <Button
              size="icon-sm"
              variant="ghost"
              disabled={navigationLocked || !view}
              aria-label={t('files.markdown')}
              onClick={() => void exportFile('markdown')}
            >
              <Download />
            </Button>
          </Tooltip>
          <Tooltip content={t('files.restore')}>
            <Button size="icon-sm" variant="ghost" disabled={navigationLocked || !view} aria-label={t('files.restore')} onClick={() => void chooseRestore()}>
              <Upload />
            </Button>
          </Tooltip>
          <div className="@min-[50rem]:hidden">
            <SegmentedControl
              size="sm"
              value={pane}
              onValueChange={setPane}
              aria-label={t('panes.label')}
              options={[
                { value: 'plants', label: t('panes.plants') },
                { value: 'journal', label: t('panes.journal') },
              ]}
            />
          </div>
        </div>
      </div>
      <div className="grid min-h-0 flex-1 grid-cols-1 gap-3 @min-[50rem]:grid-cols-[240px_minmax(0,1fr)]">
        <div className={`${pane === 'plants' ? 'flex' : 'hidden'} min-h-0 flex-col @min-[50rem]:flex`}>
          <Panel className="min-h-0 flex-1" title={t('plants.title')} bodyClassName="flex h-full min-h-0 flex-col gap-3 p-3">
            <div className="flex shrink-0 gap-1">
              <Input
                size="sm"
                wrapperClassName="min-w-0 flex-1"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                disabled={navigationLocked}
                placeholder={t('plants.search')}
                aria-label={t('plants.search')}
                maxLength={160}
              />
              <Button
                size="sm"
                variant="outline"
                disabled={navigationLocked}
                onClick={() => {
                  void begin('inspect', undefined, false, { ...activeQuery.current, search, plantsOffset: 0 })
                }}
              >
                {t('plants.find')}
              </Button>
            </div>
            <Select
              size="sm"
              value={query.filter}
              disabled={navigationLocked}
              onValueChange={(filter) => {
                void begin('inspect', undefined, false, { ...activeQuery.current, filter, plantsOffset: 0 })
              }}
              aria-label={t('plants.filter')}
              options={['active', 'archived', 'all'].map((value) => ({ value, label: t(`filter.${value}`) }))}
            />
            <div className="min-h-0 flex-1 space-y-2 overflow-auto">
              {view?.plants.items.map((p) => (
                <button
                  key={p.id}
                  type="button"
                  disabled={navigationLocked}
                  onClick={() => {
                    setPane('journal')
                    void begin('apply', { type: 'select', plant: p.id }, false, { ...activeQuery.current, section: 'plans', offset: 0 })
                  }}
                  className={cn(
                    'flex w-full items-center gap-2 rounded-card border p-2.5 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring',
                    p.id === plant?.id ? 'border-primary/30 bg-primary-soft' : 'border-border bg-surface',
                  )}
                >
                  {p.thumbnail ? (
                    <img src={p.thumbnail} alt={p.name} className="size-10 shrink-0 rounded-control object-cover" />
                  ) : (
                    <Leaf className="m-2 size-6 shrink-0 text-primary" />
                  )}
                  <span className="min-w-0">
                    <span className="block truncate text-sm font-medium text-fg">{p.name}</span>
                    <span className="mt-1 block truncate text-[11px] text-fg-subtle">{p.location || p.variety || t('plants.unnamedLocation')}</span>
                  </span>
                </button>
              ))}
              {view?.plants.total === 0 && <p className="py-4 text-center text-xs text-fg-subtle">{t('plants.empty')}</p>}
            </div>
            {view && <PageBar page={view.plants} busy={navigationLocked} change={(offset) => changePage(offset, true)} />}
            <Button size="sm" variant="primary" disabled={navigationLocked || !view} onClick={() => open({ kind: 'plant' })}>
              <Plus />
              {t('plants.add')}
            </Button>
          </Panel>
        </div>
        <div className={`${pane === 'journal' ? 'flex' : 'hidden'} min-h-0 flex-col @min-[50rem]:flex`}>
          <Panel
            className="min-h-0 flex-1"
            icon={<Leaf />}
            title={
              query.section === 'today' && !editor && !photoView ? t('journal.todayTitle') : (editor?.care?.plantName ?? plant?.name ?? t('journal.title'))
            }
            extra={plant?.archived && <Badge variant="neutral">{t('filter.archived')}</Badge>}
            bodyClassName="flex h-full min-h-0 flex-col gap-3 p-3"
            actions={
              plant &&
              query.section !== 'today' && (
                <>
                  <Tooltip content={t('plants.edit')}>
                    <Button
                      size="icon-sm"
                      variant="ghost"
                      aria-label={t('plants.edit')}
                      disabled={navigationLocked}
                      onClick={() => open({ kind: 'editPlant' })}
                    >
                      <Pencil />
                    </Button>
                  </Tooltip>
                  <Tooltip content={t(plant.archived ? 'plants.unarchive' : 'plants.archive')}>
                    <Button
                      size="icon-sm"
                      variant="ghost"
                      aria-label={t(plant.archived ? 'plants.unarchive' : 'plants.archive')}
                      disabled={navigationLocked}
                      onClick={() => confirm({ type: 'archive', plant: plant.id, archived: !plant.archived })}
                    >
                      <Archive />
                    </Button>
                  </Tooltip>
                  <Tooltip content={t('plants.delete')}>
                    <Button
                      size="icon-sm"
                      variant="ghost"
                      aria-label={t('plants.delete')}
                      disabled={navigationLocked}
                      onClick={() => confirm({ type: 'deletePlant', plant: plant.id, confirm: true })}
                    >
                      <Trash2 />
                    </Button>
                  </Tooltip>
                </>
              )
            }
          >
            {[task.error, exporting.error, photoTask.error].map(
              (error, i) =>
                error && (
                  <p key={i} role="alert" className="shrink-0 text-xs text-danger">
                    {errorMessage(error)}
                  </p>
                ),
            )}
            {saveError && (
              <div className="shrink-0 space-y-2 rounded-control border border-danger/30 bg-danger/5 p-3">
                <p role="alert" className="text-xs text-danger">
                  {t('save.failed')} {saveError}
                </p>
                <Button
                  size="sm"
                  variant="outline"
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
            {confirmation ? (
              <div className="flex min-h-0 flex-1 flex-col justify-center gap-3 rounded-card border border-warning/30 bg-warning/5 p-5">
                <h3 className="text-base font-semibold text-fg">{t(`confirm.${confirmation.kind}`)}</h3>
                <p className="text-sm leading-relaxed text-fg-muted">{t(`confirm.${confirmation.kind}Hint`)}</p>
                <div className="flex gap-2">
                  <Button
                    variant={['deletePlant', 'removePhoto', 'restore'].includes(confirmation.kind) ? 'danger' : 'primary'}
                    disabled={locked || saveError !== null}
                    onClick={() => {
                      if (confirmation.kind === 'restore') void begin('restore', undefined, true, emptyQuery, confirmation.path)
                      else if (confirmation.op) action(confirmation.op, true)
                    }}
                  >
                    {t('common.confirm')}
                  </Button>
                  <Button variant="ghost" disabled={locked || saveError !== null} onClick={() => setConfirmation(null)}>
                    {t('common.cancel')}
                  </Button>
                </div>
              </div>
            ) : editor && view ? (
              <Form
                key={`${editor.kind}-${editor.care?.id ?? ''}`}
                editor={editor}
                view={view}
                busy={locked || saveError !== null}
                previewDue={previewDue}
                clearPreview={() => setPreviewDue(null)}
                submit={(op, preview) => {
                  void begin(preview ? 'preview' : 'apply', op, !preview)
                }}
                cancel={() => setEditor(null)}
              />
            ) : photoView ? (
              <div className="flex min-h-0 flex-1 flex-col gap-3">
                <div className="flex items-center justify-between">
                  <h3 className="text-sm font-semibold text-fg">{t('photos.comparison')}</h3>
                  <Button size="sm" variant="outline" onClick={() => setPhotoView(null)}>
                    {t('common.back')}
                  </Button>
                </div>
                <div className={cn('grid min-h-0 flex-1 gap-3 overflow-auto', photoView.length === 2 ? 'grid-cols-2' : 'grid-cols-1')}>
                  {photoView.map((photo) => (
                    <figure key={photo.id} className="flex min-h-0 flex-col gap-2 rounded-card border border-border p-3">
                      <figcaption className="text-xs font-medium tabular-nums text-fg">{photo.date}</figcaption>
                      <img src={photo.data} alt={t('photos.alt', { date: photo.date })} className="min-h-0 flex-1 object-contain" />
                      <p className="shrink-0 text-xs leading-relaxed whitespace-pre-wrap break-words text-fg-muted">{photo.note}</p>
                    </figure>
                  ))}
                </div>
                <p className="shrink-0 text-[11px] text-fg-subtle">{t('photos.normalized')}</p>
              </div>
            ) : view ? (
              <>
                <div className="flex shrink-0 flex-wrap items-center justify-between gap-2">
                  <div className={navigationLocked ? 'pointer-events-none opacity-50' : ''}>
                    <SegmentedControl
                      size="sm"
                      value={query.section}
                      onValueChange={(section) => {
                        if (!navigationLocked) changeSection(section)
                      }}
                      aria-label={t('tabs.label')}
                      options={(['today', 'plans', 'timeline', 'gallery'] as const).map((value) => ({ value, label: t(`tabs.${value}`) }))}
                    />
                  </div>
                  {query.section === 'plans' && plant && (
                    <Button size="sm" variant="outline" disabled={navigationLocked || plant.archived} onClick={() => open({ kind: 'plan' })}>
                      <Plus />
                      {t('care.add')}
                    </Button>
                  )}
                </div>
                {plant && query.section !== 'today' && (
                  <p className="shrink-0 text-xs text-fg-muted">
                    {plant.variety}
                    <span className="mx-2">·</span>
                    {plant.location}
                    {plant.note && <span className="ml-2 text-fg-subtle">{plant.note}</span>}
                  </p>
                )}
                {query.section === 'today' || query.section === 'plans' ? (
                  <CareList view={view} busy={navigationLocked} edit={open} action={action} change={changePage} />
                ) : query.section === 'timeline' ? (
                  <Timeline view={view} busy={navigationLocked} confirm={confirm} change={changePage} />
                ) : (
                  <Gallery
                    view={view}
                    busy={navigationLocked}
                    picked={picked}
                    pick={(id) => setPicked(picked.includes(id) ? picked.filter((p) => p !== id) : [...picked, id])}
                    compare={() => void comparePhotos()}
                    add={() => void choosePhoto()}
                    confirm={confirm}
                    action={action}
                    change={changePage}
                  />
                )}
              </>
            ) : (
              <div className="flex min-h-0 flex-1 items-center justify-center">
                <Empty icon={<Leaf />} title={t('save.loading')} />
              </div>
            )}
            <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-t border-border pt-2">
              <p role="status" className={cn('flex items-center gap-1.5 text-[11px]', status === 'error' ? 'text-danger' : 'text-fg-subtle')}>
                {locked && <Spinner className="size-3" />}
                {t(`save.${status}`)}
              </p>
              {(task.running || photoTask.running || exporting.running) && (
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => {
                    const current = task.running ? task : photoTask.running ? photoTask : exporting
                    void current.cancel().catch((error) => toast.error(errorMessage(error)))
                  }}
                >
                  <Square />
                  {t('common.cancel')}
                </Button>
              )}
            </div>
            {view && (
              <p className="shrink-0 text-[10px] text-fg-subtle">
                {t('storage', { bytes: view.counts.storedBytes, photos: view.counts.photos, records: view.counts.records })}
              </p>
            )}
            {task.logs.length > 0 && (
              <details className="shrink-0 rounded-control border border-border">
                <summary className="cursor-pointer px-2 py-1 text-[11px] text-fg-subtle">{t('logs.title')}</summary>
                <LogViewer entries={task.logs} className="h-20" emptyText={t('logs.empty')} jumpLabel={t('logs.latest')} />
              </details>
            )}
          </Panel>
        </div>
      </div>
    </div>
  )
}
