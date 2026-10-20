import { useEffect, useMemo, useRef, useState } from 'react'
import { Badge, Button, Checkbox, Empty, Input, Modal, Pagination, Panel, Select, Spinner, Tabs, toast } from '@toolforge/ui'
import { host, loadPluginState, savePluginState, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Archive, ArrowDown, CalendarDays, Check, ClipboardList, Copy, Download, Dumbbell, History, Plus, Save, Timer, Trash2, Undo2, Upload, X } from 'lucide-react'
import { errorCause, PersistenceError, readState, recover, request, serial, transact, type Storage } from './store'
import type { Entry, Exercise, Home, Listing, Mode, SetRecord, Status, Unit, Workout, WorkoutView } from './types'

type Tab = 'workout' | 'library' | 'history' | 'backup'
const PAGE_SIZE = 10
function Pager({ page, total, change }: { page: number; total: number; change: (value: number) => void }) {
  const { t } = usePlugin()
  return <Pagination page={page} pageSize={PAGE_SIZE} total={total} onPageChange={change} prevLabel={t('page.previous')} nextLabel={t('page.next')} pageLabel={(page) => t('page.number', { page })} totalLabel={t('page.total', { count: total })} className="p-3" />
}
function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return <label className="block min-w-0 space-y-1.5 text-xs text-fg-muted"><span>{label}</span>{children}</label>
}
function SetEditor({ record, mode, index, disabled, save }: { record: SetRecord; mode: Mode; index: number; disabled: boolean; save: (value: SetRecord) => Promise<void> }) {
  const { t } = usePlugin()
  const [draft, setDraft] = useState(record)
  const update = (key: keyof SetRecord, value: string | boolean) => setDraft((current) => ({ ...current, [key]: value }))
  const submit = (status: Status) => save({ ...draft, status })
  return <div className="space-y-2 rounded-control border border-border bg-surface p-3">
    <div className="flex flex-wrap items-center gap-2">
      <Badge variant={record.status === 'done' ? 'success' : record.status === 'skipped' ? 'warning' : 'neutral'}>{t('set.number', { number: index + 1 })}</Badge>
      <span className="text-xs text-fg-subtle">{t(`status.${record.status}`)}</span>
      <Checkbox checked={draft.warmup} onCheckedChange={(value) => update('warmup', value)} disabled={disabled} className="ml-auto text-xs">{t('set.warmup')}</Checkbox>
    </div>
    <div className="grid grid-cols-[minmax(70px,1fr)_minmax(90px,1fr)_80px] gap-2">
      <Field label={t('set.reps')}><Input value={draft.reps} onChange={(e) => update('reps', e.target.value)} inputMode="numeric" disabled={disabled} wrapperClassName="w-full" aria-label={t('set.reps')} /></Field>
      <Field label={t('set.weight')}><Input value={draft.weight} onChange={(e) => update('weight', e.target.value)} inputMode="decimal" disabled={disabled || mode !== 'external'} placeholder={mode === 'external' ? t('set.weightPlaceholder') : t(`mode.${mode}`)} wrapperClassName="w-full" aria-label={t('set.weight')} /></Field>
      <Field label={t('set.unit')}><Select<Unit> value={draft.unit} onValueChange={(value) => update('unit', value)} options={[{ value: 'kg', label: t('unit.kg') }, { value: 'lb', label: t('unit.lb') }]} disabled={disabled || mode !== 'external'} className="w-full" aria-label={t('set.unit')} /></Field>
    </div>
    <Input value={draft.note} onChange={(e) => update('note', e.target.value)} disabled={disabled} placeholder={t('set.note')} aria-label={t('set.note')} wrapperClassName="w-full" />
    <div className="flex flex-wrap gap-1.5">
      <Button size="sm" variant="primary" disabled={disabled} onClick={() => void submit('done')}><Check />{t('set.complete')}</Button>
      <Button size="sm" variant="outline" disabled={disabled} onClick={() => void submit(draft.status)}><Save />{t('set.save')}</Button>
      <Button size="sm" variant="ghost" disabled={disabled} onClick={() => void submit('skipped')}><ArrowDown />{t('set.skip')}</Button>
      {record.status !== 'pending' && <Button size="sm" variant="ghost" disabled={disabled} onClick={() => void submit('pending')}>{t('set.pending')}</Button>}
    </div>
  </div>
}
function RestTimer({ workout, run, disabled }: { workout: Workout; run: (action: object) => Promise<void>; disabled: boolean }) {
  const { t, call } = usePlugin()
  const [seconds, setSeconds] = useState('90')
  const [remaining, setRemaining] = useState<number | null>(null)
  useEffect(() => {
    let alive = true
    const tick = () => void call<{ remaining: number | null }>('clock', { rest: workout.body.rest }).then((value) => { if (alive) setRemaining(value.remaining) }).catch(() => {})
    tick()
    const timer = setInterval(tick, 1000)
    return () => { alive = false; clearInterval(timer) }
  }, [call, workout.body.rest])
  return <div className="flex flex-wrap items-end gap-2 rounded-control bg-surface-2 p-3">
    <Timer className="mb-2 size-4 text-fg-subtle" />
    <Field label={t('rest.seconds')}><Input value={seconds} inputMode="numeric" onChange={(e) => setSeconds(e.target.value)} aria-label={t('rest.seconds')} wrapperClassName="w-24" /></Field>
    <Button size="sm" disabled={disabled} onClick={() => void run({ op: 'rest', workout: workout.id, seconds, stop: false })}>{t('rest.start')}</Button>
    <Button size="sm" disabled={disabled || remaining === null} variant="ghost" onClick={() => void run({ op: 'rest', workout: workout.id, seconds: '1', stop: true })}>{t('rest.stop')}</Button>
    {remaining !== null && <span aria-live="polite" className="mb-1 text-sm font-medium tabular-nums text-primary">{t(remaining === 0 ? 'rest.finished' : 'rest.remaining', { seconds: remaining })}</span>}
    <p className="w-full text-[11px] text-fg-subtle">{t('rest.hint')}</p>
  </div>
}
function EntryCard({ entry, index, workout, disabled, run }: { entry: Entry; index: number; workout: number; disabled: boolean; run: (action: object) => Promise<void> }) {
  const { t } = usePlugin()
  return <section className="space-y-2 rounded-panel border border-border p-3">
    <div className="flex flex-wrap items-center gap-2"><Dumbbell className="size-4 text-primary" /><h3 className="text-sm font-medium text-fg">{entry.exercise.name}</h3><span className="text-xs text-fg-subtle">{entry.exercise.equipment}</span><Badge>{t(`mode.${entry.exercise.mode}`)}</Badge></div>
    <div className="grid gap-2 xl:grid-cols-2">{entry.sets.map((record, set) => <SetEditor key={`${set}:${record.status}:${record.reps}:${record.weight}:${record.unit}:${record.warmup}:${record.note}`} record={record} mode={entry.exercise.mode} index={set} disabled={disabled} save={(value) => run({ op: 'set', workout, entry: index, index: set, value })} />)}</div>
    <div className="flex gap-2"><Button size="sm" variant="soft" disabled={disabled} onClick={() => void run({ op: 'addSet', workout, entry: index, copy: false })}><Plus />{t('set.add')}</Button><Button size="sm" variant="ghost" disabled={disabled || entry.sets.length === 0} onClick={() => void run({ op: 'addSet', workout, entry: index, copy: true })}><Copy />{t('set.copy')}</Button></div>
  </section>
}
function WorkoutEditor({ view, home, disabled, run, saveCsv }: { view: WorkoutView; home: Home; disabled: boolean; run: (action: object) => Promise<void>; saveCsv: (workout: Workout) => Promise<void> }) {
  const { t } = usePlugin()
  const w = view.workout
  const [details, setDetails] = useState({ name: w.body.name, date: w.body.date, note: w.body.note })
  const [exercise, setExercise] = useState('')
  const [template, setTemplate] = useState('')
  const [replaceTemplate, setReplaceTemplate] = useState('')
  const [finish, setFinish] = useState(false)
  return <div className="space-y-3">
    <div className="flex flex-wrap items-center gap-2"><Badge variant={w.body.finished ? 'success' : 'info'}>{t(w.body.finished ? 'workout.finished' : 'workout.draft')}</Badge><Badge>{t('workout.completedSets', { count: view.summary.completed })}</Badge><Badge>{t('workout.repetitions', { count: view.summary.reps })}</Badge><Button size="sm" variant="ghost" className="ml-auto" disabled={disabled || !w.undo} onClick={() => void run({ op: 'undo', workout: w.id })}><Undo2 />{t('workout.undo')}</Button><Button size="sm" variant="outline" disabled={disabled} onClick={() => void saveCsv(w)}><Download />{t('backup.csv')}</Button></div>
    <div className="grid gap-2 sm:grid-cols-[1fr_180px]"><Field label={t('workout.name')}><Input value={details.name} onChange={(e) => setDetails({ ...details, name: e.target.value })} aria-label={t('workout.name')} wrapperClassName="w-full" /></Field><Field label={t('workout.date')}><Input type="date" value={details.date} onChange={(e) => setDetails({ ...details, date: e.target.value })} aria-label={t('workout.date')} wrapperClassName="w-full" /></Field></div>
    <Input value={details.note} onChange={(e) => setDetails({ ...details, note: e.target.value })} placeholder={t('workout.note')} aria-label={t('workout.note')} wrapperClassName="w-full" />
    <Button size="sm" variant="outline" disabled={disabled} onClick={() => void run({ op: 'details', workout: w.id, ...details })}><Save />{t('workout.saveDetails')}</Button>
    <RestTimer workout={w} run={run} disabled={disabled} />
    <div className="flex flex-wrap items-center gap-2"><Select value={exercise} onValueChange={setExercise} options={home.choices.map((e) => ({ value: e.id, label: e.equipment ? `${e.name} · ${e.equipment}` : e.name }))} placeholder={t('exercise.choose')} className="min-w-48" aria-label={t('exercise.choose')} /><Button size="sm" disabled={disabled || !exercise} onClick={() => void run({ op: 'addExercise', workout: w.id, exercise })}><Plus />{t('exercise.addToWorkout')}</Button></div>
    {w.body.entries.length === 0 && <Empty icon={<Dumbbell />} title={t('workout.empty')} description={t('workout.emptyHint')} />}
    {w.body.entries.map((entry, index) => <EntryCard key={entry.exercise.id} entry={entry} index={index} workout={w.id} disabled={disabled} run={run} />)}
    <div className="space-y-2 rounded-control border border-border p-3"><p className="text-xs text-fg-subtle">{t('template.hint')}</p><div className="flex flex-wrap gap-2"><Input value={template} onChange={(e) => setTemplate(e.target.value)} placeholder={t('template.name')} aria-label={t('template.name')} wrapperClassName="min-w-40 flex-1" /><Select value={replaceTemplate} onValueChange={setReplaceTemplate} options={[{ value: '', label: t('template.create') }, ...home.templateChoices.map((t) => ({ value: t.id, label: t.name }))]} aria-label={t('template.destination')} /><Button size="sm" disabled={disabled || !template || w.body.entries.length === 0} onClick={() => void run({ op: 'saveTemplate', workout: w.id, id: replaceTemplate || null, name: template })}><Save />{t('template.save')}</Button></div></div>
    <Button variant="primary" disabled={disabled || w.body.finished} onClick={() => setFinish(true)}><Check />{t('workout.finish')}</Button>
    <Modal open={finish} onOpenChange={setFinish} title={t('workout.review')} description={t('workout.reviewHint', { count: view.summary.pending })} closeLabel={t('common.cancel')} footer={<><Button onClick={() => setFinish(false)}>{t('common.cancel')}</Button><Button variant="primary" disabled={disabled} onClick={() => { setFinish(false); void run({ op: 'finish', workout: w.id }) }}>{t('workout.confirmFinish')}</Button></>}><div className="space-y-1 px-5 pb-4">{w.body.entries.map((entry) => <p key={entry.exercise.id} className="text-xs text-fg-muted">{entry.exercise.name} · {t('workout.allSets', { count: entry.sets.length })}</p>)}</div></Modal>
  </div>
}
function WorkoutLogTool() {
  const { t, call, manifest, errorMessage } = usePlugin()
  const storage = useMemo<Storage>(() => ({ read: (key) => loadPluginState(manifest.id, key), write: (key, value) => savePluginState(manifest.id, key, value), call }), [call, manifest.id])
  const [tab, setTab] = useState<Tab>('workout')
  const [home, setHome] = useState<Home | null>(null)
  const [listing, setListing] = useState<Listing | null>(null)
  const [view, setView] = useState<WorkoutView | null>(null)
  const [selected, setSelected] = useState<number | null>(null)
  const [revision, setRevision] = useState(0)
  const [page, setPage] = useState(1)
  const [libraryPage, setLibraryPage] = useState(1)
  const [templatePage, setTemplatePage] = useState(1)
  const [filterExercise, setFilterExercise] = useState('')
  const [filterDate, setFilterDate] = useState('')
  const [warmups, setWarmups] = useState(false)
  const [busy, setBusy] = useState(false)
  const gate = useRef(false)
  const [failure, setFailure] = useState<string | null>(null)
  const [message, setMessage] = useState<string | null>(null)
  const [start, setStart] = useState(false)
  const [startName, setStartName] = useState('')
  const [startDate, setStartDate] = useState('')
  const [startTemplate, setStartTemplate] = useState('')
  const [exerciseDraft, setExerciseDraft] = useState<{ id: string | null; name: string; equipment: string; mode: Mode } | null>(null)
  const [confirmArchive, setConfirmArchive] = useState<Exercise | null>(null)
  const [confirmTemplate, setConfirmTemplate] = useState<string | null>(null)
  const [importing, setImporting] = useState(false)
  const [progress, setProgress] = useState<string | null>(null)

  useEffect(() => {
    let alive = true
    void Promise.all([
      transact<Home>(manifest.id, storage, { op: 'home', offset: (libraryPage - 1) * PAGE_SIZE, templateOffset: (templatePage - 1) * PAGE_SIZE, limit: PAGE_SIZE }),
      transact<Listing>(manifest.id, storage, { op: 'list', offset: (page - 1) * PAGE_SIZE, limit: PAGE_SIZE, exercise: filterExercise || null, date: filterDate || null, warmups }),
    ]).then(([nextHome, nextListing]) => { if (alive) {
      setHome(nextHome); setListing(nextListing); setFailure(null)
      setTemplatePage((current) => Math.min(current, Math.max(1, Math.ceil(nextHome.templateTotal / PAGE_SIZE))))
    } }).catch((e) => { if (alive) setFailure(errorMessage(errorCause(e))) })
    return () => { alive = false }
  }, [storage, manifest.id, errorMessage, page, libraryPage, templatePage, filterExercise, filterDate, warmups, revision])
  useEffect(() => {
    if (selected === null) return
    let alive = true
    void transact<WorkoutView>(manifest.id, storage, { op: 'get', workout: selected, warmups }).then((next) => { if (alive) setView(next) }).catch((e) => { if (alive) setFailure(errorMessage(errorCause(e))) })
    return () => { alive = false }
  }, [storage, manifest.id, errorMessage, selected, warmups, revision])

  const showError = (error: unknown) => {
    const text = errorMessage(errorCause(error))
    setMessage(text)
    if (error instanceof PersistenceError) setFailure(text)
    toast.error(text)
  }
  const run = async (action: object) => {
    if (gate.current || failure) return
    gate.current = true; setBusy(true); setMessage(null)
    try {
      const result = await transact<Partial<WorkoutView>>(manifest.id, storage, action)
      if (result.workout) { setSelected(result.workout.id); setView(result as WorkoutView) }
      setRevision((n) => n + 1)
      setExerciseDraft(null); setStart(false)
    } catch (e) { showError(e) }
    finally { gate.current = false; setBusy(false) }
  }
  const begin = () => { setStartName(t('workout.defaultName')); setStartDate(home?.today ?? ''); setStartTemplate(''); setStart(true) }
  const open = (row: WorkoutView) => { setSelected(row.workout.id); setView(row); setTab('workout') }
  const exportCsv = async (workout: Workout) => {
    if (gate.current) return
    const path = await host.dialog.saveFile('workout.csv', [{ name: t('backup.csvFilter'), extensions: ['csv'] }])
    if (!path) return
    gate.current = true; setBusy(true)
    try { await serial(manifest.id, async () => {
      await recover(storage)
      const head = await readState(storage, 'head')
      const current = await request<WorkoutView>(storage, { op: 'get', workout: workout.id, warmups: false })
      const output = await call<{ content: string }>('export', { head, workouts: [current.workout], format: 'csv', offset: 0, directory: null, stamp: null })
      await host.fs.writeText(path, output.content)
    }); toast.success(t('backup.saved')) } catch (e) { showError(e) }
    finally { gate.current = false; setBusy(false) }
  }
  const backup = async () => {
    if (gate.current) return
    const directory = await host.dialog.openDirectory()
    if (!directory) return
    gate.current = true; setBusy(true); setMessage(null)
    try { await serial(manifest.id, async () => {
      await recover(storage)
      const head = await readState(storage, 'head')
      const info = await request<Home>(storage, { op: 'home', offset: 0, limit: 1 })
      const clock = await call<{ stamp: string }>('clock', { rest: null })
      for (let offset = 0; offset < Math.max(1, info.workoutTotal); offset++) {
        const current = info.workoutTotal ? await request<WorkoutView>(storage, { op: 'get', workout: offset + 1, warmups: false }) : null
        const output = await call<{ content: string; path: string }>('export', { head, workouts: current ? [current.workout] : [], format: 'json', offset, directory, stamp: clock.stamp })
        await host.fs.writeText(output.path, output.content)
        setProgress(t('backup.progress', { done: offset + 1, total: Math.max(1, info.workoutTotal) }))
      }
    }); toast.success(t('backup.saved')) } catch (e) { showError(e) }
    finally { gate.current = false; setBusy(false); setProgress(null) }
  }
  const restore = async () => {
    setImporting(false)
    if (gate.current) return
    const files = await host.dialog.openFiles([{ name: t('backup.jsonFilter'), extensions: ['json'] }])
    if (!files.length) return
    gate.current = true; setBusy(true); setMessage(null)
    try { await serial(manifest.id, async () => {
      await recover(storage)
      for (let index = 0; index < files.length; index++) {
        const text = await host.fs.readText(files[index])
        await request(storage, {}, 'import', { text })
        setProgress(t('backup.progress', { done: index + 1, total: files.length }))
      }
    }); setSelected(null); setView(null); setPage(1); setLibraryPage(1); setTemplatePage(1); setRevision((n) => n + 1); toast.success(t('backup.restored')) }
    catch (e) { showError(e); if (!(e instanceof PersistenceError)) setRevision((n) => n + 1) }
    finally { gate.current = false; setBusy(false); setProgress(null) }
  }
  const disabled = busy || failure !== null
  return <div className="flex h-full min-h-0 flex-col gap-3">
    <div className="flex flex-wrap items-center gap-2"><Tabs<Tab> value={tab} onValueChange={setTab} aria-label={t('name')} items={[{ value: 'workout', label: t('tabs.workout'), icon: <Dumbbell /> }, { value: 'library', label: t('tabs.library'), icon: <ClipboardList /> }, { value: 'history', label: t('tabs.history'), icon: <History /> }, { value: 'backup', label: t('tabs.backup'), icon: <Download /> }]} /><Button variant="primary" size="sm" className="ml-auto" disabled={disabled || !home} onClick={begin}><Plus />{t('workout.start')}</Button></div>
    {(failure || message) && <div role="alert" className="flex items-center gap-2 rounded-control border border-danger/30 bg-danger-soft p-3 text-xs text-danger"><p className="flex-1">{message ?? failure}</p><Button size="sm" disabled={busy} onClick={() => { setMessage(null); setRevision((n) => n + 1) }}>{t('common.retry')}</Button></div>}
    {!home ? <Empty icon={<Spinner />} title={t('common.loading')} /> : <div className="min-h-0 flex-1 overflow-auto">
      {tab === 'workout' && <Panel title={view?.workout.body.name ?? t('workout.title')} icon={<Dumbbell />} bodyClassName="p-3"><Checkbox checked={warmups} onCheckedChange={setWarmups} className="mb-3 text-xs">{t('history.includeWarmups')}</Checkbox>{view ? <WorkoutEditor key={`${view.workout.id}:${view.workout.body.date}:${view.workout.body.name}:${view.workout.body.note}`} view={view} home={home} disabled={disabled} run={run} saveCsv={exportCsv} /> : <><Empty icon={<Dumbbell />} title={t('workout.welcome')} description={t('workout.welcomeHint')} />{listing?.rows.map((row) => <Button key={row.workout.id} block variant="ghost" className="justify-start" onClick={() => open(row)}>{row.workout.body.date} · {row.workout.body.name}<Badge className="ml-auto">{t(row.workout.body.finished ? 'workout.finished' : 'workout.draft')}</Badge></Button>)}<Pager page={page} total={listing?.total ?? 0} change={setPage} /></>}</Panel>}
      {tab === 'library' && <div className="grid gap-3 lg:grid-cols-2"><Panel title={t('exercise.title')} icon={<Dumbbell />} actions={<Button size="sm" variant="soft" disabled={disabled} onClick={() => setExerciseDraft({ id: null, name: '', equipment: '', mode: 'external' })}><Plus />{t('exercise.create')}</Button>} bodyClassName="p-3"><p className="mb-3 text-xs text-fg-subtle">{t('exercise.hint')}</p>{home.exercises.length === 0 && <Empty title={t('exercise.empty')} />}{home.exercises.map((exercise) => <div key={exercise.id} className="mb-2 rounded-control border border-border p-3"><div className="flex gap-2"><div className="min-w-0 flex-1"><p className="truncate text-sm font-medium">{exercise.name}</p><p className="text-xs text-fg-subtle">{exercise.equipment} · {t(`mode.${exercise.mode}`)}</p></div>{exercise.archived && <Badge>{t('exercise.archived')}</Badge>}</div><div className="mt-2 flex gap-1"><Button size="sm" variant="ghost" disabled={disabled} onClick={() => setExerciseDraft({ id: exercise.id, name: exercise.name, equipment: exercise.equipment, mode: exercise.mode })}>{t('exercise.edit')}</Button><Button size="sm" variant="ghost" disabled={disabled} onClick={() => setConfirmArchive(exercise)}><Archive />{t(exercise.archived ? 'exercise.restore' : 'exercise.archive')}</Button><Button size="sm" variant="ghost" onClick={() => { setFilterExercise(exercise.id); setFilterDate(''); setPage(1); setTab('history') }}><History />{t('exercise.history')}</Button></div></div>)}<Pager page={libraryPage} total={home.exerciseTotal} change={setLibraryPage} /></Panel><Panel title={t('template.title')} icon={<ClipboardList />} bodyClassName="p-3"><p className="mb-3 text-xs text-fg-subtle">{t('template.createHint')}</p>{home.templates.length === 0 && <Empty title={t('template.empty')} />}{home.templates.map((template) => <div key={template.id} className="mb-2 flex items-center gap-2 rounded-control border border-border p-3"><span className="min-w-0 flex-1 truncate text-sm">{template.name}</span><Badge>{t('template.exercises', { count: template.entries.length })}</Badge><Button size="sm" variant="ghost" disabled={disabled} onClick={() => { setStartName(template.name); setStartDate(home.today); setStartTemplate(template.id); setStart(true) }}><Plus />{t('template.use')}</Button><Button size="icon-sm" variant="ghost" disabled={disabled} onClick={() => setConfirmTemplate(template.id)} aria-label={t('template.delete')}><Trash2 /></Button></div>)}<Pager page={templatePage} total={home.templateTotal} change={setTemplatePage} /></Panel></div>}
      {tab === 'history' && <Panel title={t('history.title')} icon={<CalendarDays />} bodyClassName="p-3"><div className="mb-3 flex flex-wrap items-end gap-2"><Field label={t('history.exercise')}><Select value={filterExercise} onValueChange={(value) => { setFilterExercise(value); setFilterDate(''); setPage(1) }} options={[{ value: '', label: t('history.all') }, ...home.historyChoices.map((e) => ({ value: e.id, label: `${e.equipment ? `${e.name} · ${e.equipment}` : e.name}${e.archived ? ` · ${t('exercise.archived')}` : ''}` }))]} aria-label={t('history.exercise')} className="min-w-48" /></Field><Field label={t('history.date')}><Input type="date" value={filterDate} onChange={(e) => { setFilterDate(e.target.value); setFilterExercise(''); setPage(1) }} aria-label={t('history.date')} /></Field><Button size="icon-sm" variant="ghost" aria-label={t('history.clear')} onClick={() => { setFilterDate(''); setFilterExercise(''); setPage(1) }}><X /></Button><Checkbox checked={warmups} onCheckedChange={setWarmups} className="mb-1 text-xs">{t('history.includeWarmups')}</Checkbox></div><p className="mb-3 text-[11px] text-fg-subtle">{t('history.hint')}</p>{!listing?.rows.length ? <Empty title={t('history.empty')} /> : listing.rows.map((row) => <article key={row.workout.id} className="mb-3 rounded-control border border-border p-3"><div className="mb-2 flex flex-wrap items-center gap-2"><span className="font-medium text-fg">{row.workout.body.name}</span><Badge>{row.workout.body.date}</Badge><Badge>{t('workout.completedSets', { count: row.summary.completed })}</Badge><Button size="sm" variant="ghost" className="ml-auto" disabled={disabled} onClick={() => open(row)}>{t('history.open')}</Button></div>{row.summary.entries.map((entry) => <div key={entry.exercise.id} className="mb-2 rounded-control bg-surface-2 p-2 text-xs"><p className="font-medium">{entry.exercise.name} · {entry.exercise.equipment} · {t(`mode.${entry.exercise.mode}`)}</p>{entry.sets.map((set) => <p key={set.index} className="mt-1 text-fg-muted">{t('set.number', { number: set.index + 1 })} · {t('history.repCount', { value: set.reps })} · {set.kg === null ? t(`mode.${entry.exercise.mode}`) : t('history.weightValue', { original: set.weight, unit: t(`unit.${set.unit}`), kg: set.kg })}{set.warmup && <Badge className="ml-2">{t('set.warmup')}</Badge>}{set.note && <span className="ml-2">{set.note}</span>}</p>)}</div>)}</article>)}<Pager page={page} total={listing?.total ?? 0} change={setPage} /></Panel>}
      {tab === 'backup' && <Panel title={t('backup.title')} icon={<Download />} bodyClassName="space-y-4 p-4"><p className="max-w-2xl text-sm text-fg-muted">{t('backup.hint')}</p><div className="flex flex-wrap gap-2"><Button variant="primary" disabled={disabled} onClick={() => void backup()}><Download />{t('backup.export')}</Button><Button variant="outline" disabled={disabled} onClick={() => setImporting(true)}><Upload />{t('backup.import')}</Button></div>{progress && <p aria-live="polite" className="text-sm text-primary">{progress}</p>}<p className="text-xs text-fg-subtle">{t('backup.privacy')}</p></Panel>}
    </div>}
    <Modal open={start} onOpenChange={setStart} title={t('workout.start')} closeLabel={t('common.cancel')} footer={<><Button disabled={busy} onClick={() => setStart(false)}>{t('common.cancel')}</Button><Button variant="primary" disabled={disabled} onClick={() => { void run({ op: 'start', name: startName, date: startDate, template: startTemplate || null }); setTab('workout') }}>{t('workout.begin')}</Button></>}><div className="space-y-3 px-5 pb-4"><Field label={t('workout.name')}><Input autoFocus value={startName} onChange={(e) => setStartName(e.target.value)} aria-label={t('workout.name')} wrapperClassName="w-full" /></Field><Field label={t('workout.date')}><Input type="date" value={startDate} onChange={(e) => setStartDate(e.target.value)} aria-label={t('workout.date')} wrapperClassName="w-full" /></Field><Field label={t('template.choose')}><Select value={startTemplate} onValueChange={setStartTemplate} options={[{ value: '', label: t('template.blank') }, ...(home?.templateChoices.map((v) => ({ value: v.id, label: v.name })) ?? [])]} aria-label={t('template.choose')} className="w-full" /></Field></div></Modal>
    <Modal open={exerciseDraft !== null} onOpenChange={(open) => { if (!open) setExerciseDraft(null) }} title={t(exerciseDraft?.id ? 'exercise.edit' : 'exercise.create')} closeLabel={t('common.cancel')} footer={<><Button disabled={busy} onClick={() => setExerciseDraft(null)}>{t('common.cancel')}</Button><Button variant="primary" disabled={disabled} onClick={() => exerciseDraft && void run({ op: 'exercise', ...exerciseDraft })}>{t('common.save')}</Button></>}><div className="space-y-3 px-5 pb-4">{exerciseDraft && <><Field label={t('exercise.name')}><Input autoFocus value={exerciseDraft.name} onChange={(e) => setExerciseDraft({ ...exerciseDraft, name: e.target.value })} aria-label={t('exercise.name')} wrapperClassName="w-full" /></Field><Field label={t('exercise.equipment')}><Input value={exerciseDraft.equipment} onChange={(e) => setExerciseDraft({ ...exerciseDraft, equipment: e.target.value })} aria-label={t('exercise.equipment')} wrapperClassName="w-full" /></Field><Field label={t('exercise.mode')}><Select<Mode> value={exerciseDraft.mode} onValueChange={(mode) => setExerciseDraft({ ...exerciseDraft, mode })} options={(['external', 'bodyweight', 'unknown'] as Mode[]).map((value) => ({ value, label: t(`mode.${value}`) }))} aria-label={t('exercise.mode')} className="w-full" /></Field><p className="text-xs text-fg-subtle">{t('exercise.snapshotHint')}</p></>}</div></Modal>
    <Modal open={confirmArchive !== null} onOpenChange={(open) => { if (!open) setConfirmArchive(null) }} title={t('exercise.archiveTitle')} description={t('exercise.archiveHint')} closeLabel={t('common.cancel')} footer={<><Button onClick={() => setConfirmArchive(null)}>{t('common.cancel')}</Button><Button variant="primary" disabled={disabled} onClick={() => { if (confirmArchive) void run({ op: 'archive', id: confirmArchive.id }); setConfirmArchive(null) }}>{t('common.confirm')}</Button></>} />
    <Modal open={confirmTemplate !== null} onOpenChange={(open) => { if (!open) setConfirmTemplate(null) }} title={t('template.delete')} description={t('template.deleteHint')} closeLabel={t('common.cancel')} footer={<><Button onClick={() => setConfirmTemplate(null)}>{t('common.cancel')}</Button><Button variant="danger" disabled={disabled} onClick={() => { if (confirmTemplate) void run({ op: 'deleteTemplate', id: confirmTemplate }); setConfirmTemplate(null) }}>{t('common.confirm')}</Button></>} />
    <Modal open={importing} onOpenChange={setImporting} title={t('backup.import')} description={t('backup.importHint')} closeLabel={t('common.cancel')} footer={<><Button onClick={() => setImporting(false)}>{t('common.cancel')}</Button><Button variant="primary" disabled={disabled} onClick={() => void restore()}>{t('backup.chooseFiles')}</Button></>} />
  </div>
}
export default WorkoutLogTool
