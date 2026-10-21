import { useEffect, useRef, useState } from 'react'
import { Badge, Button, Empty, LogViewer, Panel, SegmentedControl, Spinner, Tooltip, cn, toast } from '@toolforge/ui'
import { host, loadPluginState, savePluginState, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { BookOpen, Copy, Download, FileJson, Layers, Pencil, Plus, RotateCcw, Scissors, Square, Trash2, Upload } from 'lucide-react'
import { Editor } from './Editor'
import { counterKeyAction } from './keyboard'
import { HistoryView, PageBar, ProgressView, RulesView } from './ProgressView'
import { emptyPages, emptyWorkshop, type Exported, type FormKind, type Op, type Pages, type Result, type View, type Workshop } from './types'

type Confirmation = { kind: 'clear' | 'part' | 'project' | 'segment' | 'reminder' | 'restore'; op?: Op; path?: string }

export default function Knitting() {
  const { t, manifest, errorMessage } = usePlugin()
  const task = useTask<Result>()
  const exporting = useTask<Exported>()
  const [view, setView] = useState<View | null>(null)
  const [acting, setActing] = useState(false)
  const [exportBusy, setExportBusy] = useState(false)
  const [status, setStatus] = useState('loading')
  const [saveError, setSaveError] = useState<string | null>(null)
  const [retry, setRetry] = useState(0)
  const [editor, setEditor] = useState<FormKind | null>(null)
  const [confirmation, setConfirmation] = useState<Confirmation | null>(null)
  const [tab, setTab] = useState<'counter' | 'rules' | 'history'>('counter')
  const source = useRef<Workshop | null>(null)
  const pages = useRef(emptyPages())
  const sequence = useRef(0)
  const currentRequest = useRef('')
  const pending = useRef(false)
  const exportPending = useRef(false)
  const handled = useRef('')
  const handledExport = useRef<string | null>(null)
  const closeOnSuccess = useRef(false)
  const mounted = useRef(true)
  const startRef = useRef(task.start)
  const errorRef = useRef(errorMessage)
  const loadRef = useRef<() => void>(() => {})
  const spaceRef = useRef<() => void>(() => {})
  const locked = acting || exportBusy
  const controlsLocked = locked || editor !== null || confirmation !== null

  const begin = async (functionName: 'inspect' | 'apply' | 'restore', extra: object = {}, options: { follow?: boolean; close?: boolean; pages?: Pages } = {}) => {
    if (pending.current || exportPending.current || !source.current) return
    pending.current = true
    const requestId = String(++sequence.current)
    currentRequest.current = requestId
    closeOnSuccess.current = options.close ?? false
    setActing(true)
    setSaveError(null)
    setStatus(functionName === 'inspect' ? 'loading' : 'saving')
    const id = await startRef.current(functionName, { state: source.current, requestId, pages: options.pages ?? pages.current, followSelection: options.follow ?? false, ...extra })
    if (id === null) { pending.current = false; setActing(false); setStatus('error') }
  }

  const reload = async () => {
    if (pending.current) return
    pending.current = true; setActing(true); setStatus('loading'); setSaveError(null)
    try {
      const saved = await loadPluginState<Workshop>(manifest.id, 'workshop')
      if (!mounted.current) return
      source.current = saved ?? emptyWorkshop()
      pending.current = false
      await begin('inspect', {}, { follow: true })
    } catch (error) {
      if (mounted.current) { pending.current = false; setActing(false); setStatus('error'); setSaveError(errorRef.current(error)) }
    }
  }

  const action = (op: Op, close = false, follow = false) => { void begin('apply', { op }, { close, follow }) }
  const page = (key: keyof Pages, offset: number) => { void begin('inspect', {}, { pages: { ...pages.current, [key]: offset } }) }
  const openEditor = (kind: FormKind) => { if (!controlsLocked && view) setEditor(kind) }
  const ask = (op: Op, kind: Confirmation['kind']) => { if (!controlsLocked) setConfirmation({ kind, op }) }
  const chooseBackup = async () => {
    try {
      const path = await host.dialog.openFile([{ name: t('files.backup'), extensions: ['json'] }])
      if (path) setConfirmation({ kind: 'restore', path })
    } catch (error) { toast.error(errorMessage(error)) }
  }
  const confirm = () => {
    if (!confirmation) return
    if (confirmation.kind === 'restore') void begin('restore', { path: confirmation.path }, { close: true, follow: true, pages: emptyPages() })
    else if (confirmation.op) action(confirmation.op, true, true)
  }

  const save = async (format: 'json' | 'markdown', copy = false) => {
    if (pending.current || exportPending.current || !source.current) return
    exportPending.current = true; setExportBusy(true); exporting.reset()
    const snapshot = source.current
    try {
      const ext = format === 'json' ? 'json' : 'md'
      const path = copy ? null : await host.dialog.saveFile(`knitting-workshop.${ext}`, [{ name: ext.toUpperCase(), extensions: [ext] }])
      if (!copy && !path) { exportPending.current = false; setExportBusy(false); return }
      const id = await exporting.start('export', { state: snapshot, format, path, language: t('locale'), partOnly: copy })
      if (id === null) { exportPending.current = false; setExportBusy(false) }
    } catch (error) { exportPending.current = false; setExportBusy(false); toast.error(errorMessage(error)) }
  }

  useEffect(() => {
    startRef.current = task.start
    errorRef.current = errorMessage
    loadRef.current = () => { void reload() }
    spaceRef.current = () => {
      if (!controlsLocked && tab === 'counter' && view?.part?.canComplete) action({ type: 'complete', expectedLine: view.part.nextLine })
    }
  })
  useEffect(() => {
    mounted.current = true; loadRef.current()
    return () => { mounted.current = false }
  }, [manifest.id])
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target
      const interactive = target instanceof Element && target.closest('input,textarea,select,button,a,[contenteditable],[role="textbox"],[role="combobox"],[role="listbox"],[role="menu"]') !== null
      const action = counterKeyAction(event, interactive)
      if (action === 'ignore') return
      event.preventDefault(); if (action === 'advance') spaceRef.current()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  useEffect(() => {
    const result = task.result
    if (!result || handled.current === result.requestId || currentRequest.current !== result.requestId) return
    handled.current = result.requestId
    Promise.resolve().then(async () => {
      if (source.current?.revision !== result.baseRevision) throw { code: 'knitting.stale_state' }
      if (result.state) await savePluginState(manifest.id, 'workshop', result.state)
      if (!mounted.current) return
      if (result.state) source.current = result.state
      pages.current = result.view.pages
      setView(result.view); setStatus(result.state ? 'saved' : 'ready'); setSaveError(null)
      if (closeOnSuccess.current) { setEditor(null); setConfirmation(null) }
    }).catch((error) => {
      if (mounted.current) { handled.current = ''; setSaveError(errorRef.current(error)); setStatus('error') }
    }).finally(() => { if (mounted.current) { pending.current = false; setActing(false) } })
  }, [task.result, manifest.id, retry])
  useEffect(() => {
    if (task.status !== 'failed' && task.status !== 'cancelled') return
    Promise.resolve().then(() => { if (mounted.current) { pending.current = false; setActing(false); setStatus(task.status === 'cancelled' ? 'ready' : 'error') } })
  }, [task.status])
  useEffect(() => {
    const id = exporting.taskId
    if (!id || exporting.running || exporting.status === 'idle' || handledExport.current === id) return
    handledExport.current = id
    Promise.resolve().then(async () => {
      if (exporting.result?.content !== undefined) { await host.clipboard.writeText(exporting.result.content); toast.success(t('files.copied')) }
      else if (exporting.result?.saved) toast.success(t('files.saved'))
    }).catch((error) => toast.error(errorRef.current(error))).finally(() => { exportPending.current = false; if (mounted.current) setExportBusy(false) })
  }, [exporting.taskId, exporting.running, exporting.status, exporting.result, t])

  const part = view?.part
  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(230px,0.55fr)_minmax(0,1.45fr)] gap-3">
      <div className="flex min-h-0 flex-col gap-3">
        <Panel title={t('projects.title')} icon={<BookOpen />} className="min-h-0 flex-1" bodyClassName="flex h-full min-h-0 flex-col gap-2 p-3" actions={<Tooltip content={t('projects.add')}><Button size="icon-sm" variant="ghost" disabled={controlsLocked || !view} aria-label={t('projects.add')} onClick={() => openEditor('project')}><Plus /></Button></Tooltip>}>
          <div className="min-h-0 flex-1 space-y-1 overflow-auto">{view?.projects.items.map((p) => <button key={p.id} type="button" disabled={controlsLocked} onClick={() => action({ type: 'selectProject', id: p.id }, false, true)} className={cn('block w-full rounded-control border p-2.5 text-left outline-none transition-colors hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50', p.id === view.activeProjectId ? 'border-primary/30 bg-primary-soft' : 'border-transparent')}><span className="block truncate text-sm font-medium text-fg">{p.name}</span><span className="mt-1 block text-[11px] text-fg-muted">{t('projects.parts', { count: p.partCount })}</span></button>)}{view?.projects.total === 0 && <p className="py-3 text-xs text-fg-muted">{t('projects.empty')}</p>}</div>
          {view && <PageBar page={view.projects} change={(offset) => page('projects', offset)} />}
        </Panel>
        <Panel title={t('part.title')} icon={<Layers />} className="min-h-0 flex-1" bodyClassName="flex h-full min-h-0 flex-col gap-2 p-3" actions={<Tooltip content={t('part.add')}><Button size="icon-sm" variant="ghost" disabled={controlsLocked || !view?.activeProjectId} aria-label={t('part.add')} onClick={() => openEditor('part')}><Plus /></Button></Tooltip>}>
          <div className="min-h-0 flex-1 space-y-1 overflow-auto">{view?.parts.items.map((p) => <button key={p.id} type="button" disabled={controlsLocked} onClick={() => action({ type: 'selectPart', id: p.id }, false, true)} className={cn('block w-full rounded-control border p-2.5 text-left outline-none transition-colors hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50', p.id === part?.id ? 'border-primary/30 bg-primary-soft' : 'border-transparent')}><span className="block truncate text-sm font-medium text-fg">{p.name}</span><span className="mt-1 block text-[11px] text-fg-muted">{t(`part.progress.${p.kind}`, { count: p.completed, stitches: p.stitches })}</span></button>)}{view?.parts.total === 0 && <p className="py-3 text-xs text-fg-muted">{t('part.empty')}</p>}</div>
          {view && <PageBar page={view.parts} change={(offset) => page('parts', offset)} />}
        </Panel>
        <div className="shrink-0 space-y-2 rounded-card border border-border bg-surface p-3"><div className="flex flex-wrap gap-1"><Tooltip content={t('files.json')}><Button size="icon-sm" variant="ghost" aria-label={t('files.json')} disabled={controlsLocked || !view} onClick={() => void save('json')}><FileJson /></Button></Tooltip><Tooltip content={t('files.markdown')}><Button size="icon-sm" variant="ghost" aria-label={t('files.markdown')} disabled={controlsLocked || !view} onClick={() => void save('markdown')}><Download /></Button></Tooltip><Tooltip content={t('files.copy')}><Button size="icon-sm" variant="ghost" aria-label={t('files.copy')} disabled={controlsLocked || !part} onClick={() => void save('markdown', true)}><Copy /></Button></Tooltip><Tooltip content={t('files.restore')}><Button size="icon-sm" variant="ghost" aria-label={t('files.restore')} disabled={controlsLocked || !view} onClick={() => void chooseBackup()}><Upload /></Button></Tooltip></div><p role="status" className={cn('flex items-center gap-1.5 text-[11px]', status === 'error' ? 'text-danger' : 'text-fg-muted')}>{locked && <Spinner className="size-3" />}{t(`save.${status}`)}</p></div>
      </div>
      <Panel icon={<Scissors />} title={part?.name ?? t('counter.title')} extra={part && <Badge variant="neutral">{t(`mode.${part.kind}`)}</Badge>} bodyClassName="flex h-full min-h-0 flex-col gap-3 p-3"
        actions={part && <><Tooltip content={t('actions.details')}><Button size="icon-sm" variant="ghost" aria-label={t('actions.details')} disabled={controlsLocked} onClick={() => openEditor('details')}><Pencil /></Button></Tooltip><Tooltip content={t('actions.adjust')}><Button size="icon-sm" variant="ghost" aria-label={t('actions.adjust')} disabled={controlsLocked} onClick={() => openEditor('adjust')}><Plus /></Button></Tooltip><Tooltip content={t('actions.copy')}><Button size="icon-sm" variant="ghost" aria-label={t('actions.copy')} disabled={controlsLocked} onClick={() => openEditor('copy')}><Copy /></Button></Tooltip><Tooltip content={t('actions.clear')}><Button size="icon-sm" variant="ghost" aria-label={t('actions.clear')} disabled={controlsLocked} onClick={() => ask({ type: 'clear', confirm: true }, 'clear')}><RotateCcw /></Button></Tooltip><Tooltip content={t('actions.deletePart')}><Button size="icon-sm" variant="ghost" aria-label={t('actions.deletePart')} disabled={controlsLocked} onClick={() => ask({ type: 'deletePart', confirm: true }, 'part')}><Trash2 /></Button></Tooltip></>}
        footer={view?.activeProjectId && <div className="flex w-full items-center justify-between"><span className="truncate text-fg-subtle">{view.projectName}</span><Button size="sm" variant="ghost" disabled={controlsLocked} onClick={() => ask({ type: 'deleteProject', confirm: true }, 'project')}>{t('actions.deleteProject')}</Button></div>}>
        {task.error && <p role="alert" className="shrink-0 text-xs text-danger">{errorMessage(task.error)}</p>}
        {saveError && <div className="shrink-0 space-y-2"><p role="alert" className="text-xs text-danger">{saveError}</p><Button size="sm" variant="outline" disabled={locked} onClick={() => { if (task.result) { pending.current = true; setActing(true); setRetry((n) => n + 1) } else void reload() }}>{t('save.retry')}</Button></div>}
        {exporting.error && <p role="alert" className="shrink-0 text-xs text-danger">{errorMessage(exporting.error)}</p>}
        {confirmation ? <div className="flex min-h-0 flex-1 flex-col justify-center gap-3 rounded-card border border-warning/30 bg-warning/10 p-5"><h3 className="text-base font-semibold text-fg">{t(`confirm.${confirmation.kind}`)}</h3><p className="text-sm leading-relaxed text-fg-muted">{t(confirmation.kind === 'restore' ? 'confirm.restoreHint' : 'confirm.hint')}</p><div className="flex gap-2"><Button variant="danger" disabled={locked} onClick={confirm}>{t('common.confirm')}</Button><Button variant="outline" disabled={locked} onClick={() => setConfirmation(null)}>{t('common.cancel')}</Button></div></div> : editor && view ? <div className="min-h-0 flex-1"><Editor key={`${editor}-${part?.id ?? ''}`} kind={editor} view={view} busy={locked} onSave={(op, follow) => action(op, true, follow)} onCancel={() => setEditor(null)} /></div> : part ? <>
          <div className={controlsLocked ? 'pointer-events-none opacity-50' : ''}><SegmentedControl size="sm" value={tab} onValueChange={(value) => { if (!controlsLocked) setTab(value) }} aria-label={t('views.label')} options={[{ value: 'counter', label: t('views.counter') }, { value: 'rules', label: t('views.rules') }, { value: 'history', label: t('views.history') }]} /></div>
          <div className="min-h-0 flex-1">{tab === 'counter' ? <ProgressView part={part} busy={controlsLocked} action={action} page={page} /> : tab === 'rules' ? <RulesView part={part} busy={controlsLocked} page={page} remove={(op, kind) => ask(op, kind as Confirmation['kind'])} add={openEditor} /> : <HistoryView part={part} page={page} />}</div>
        </> : <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-4"><Empty icon={<Scissors />} title={t('counter.empty')} description={t('counter.emptyHint')} /><Button variant="primary" disabled={controlsLocked || !view} onClick={() => openEditor(view?.activeProjectId ? 'part' : 'project')}><Plus />{t(view?.activeProjectId ? 'part.add' : 'projects.add')}</Button></div>}
        {(task.running || exportBusy) && <div className="flex shrink-0 items-center gap-2 text-xs text-fg-muted"><Spinner className="size-3.5" /><span>{t(`stages.${exportBusy ? exporting.stage ?? 'export' : task.stage ?? 'validate'}`)}</span>{task.running && <Button size="sm" variant="ghost" onClick={() => { void task.cancel().catch((error) => toast.error(errorMessage(error))) }}><Square />{t('common.cancel')}</Button>}</div>}
        {(task.logs.length > 0 || exporting.logs.length > 0) && <details className="shrink-0 rounded-control border border-border"><summary className="cursor-pointer px-2 py-1 text-[11px] text-fg-muted">{t('logs.title')}</summary><LogViewer entries={exportBusy ? exporting.logs : task.logs} className="h-24" emptyText={t('logs.empty')} jumpLabel={t('logs.latest')} /></details>}
      </Panel>
    </div>
  )
}
