import { useEffect, useRef, useState } from 'react'
import { Boxes, CheckCheck, Download, FileJson, FolderOpen, Plus, RotateCcw, Square, Trash2 } from 'lucide-react'
import { Badge, Button, CodeEditor, Empty, Input, LogViewer, Panel, Progress, SegmentedControl, Select, Spinner, Tooltip, toast } from '@toolforge/ui'
import { callPlugin, host, usePlugin, usePluginState, useTask } from '@toolforge/plugin-ui-sdk'
import { initialProject } from './samples'
import { Results } from './Results'
import type { DraftFactor, Prepared, Project, Summary } from './types'

export default function Pairwise() {
  const { t, manifest, errorMessage } = usePlugin()
  const [project, setProject] = usePluginState<Project>('project', initialProject())
  const [preparing, setPreparing] = useState(false)
  const [inputError, setInputError] = useState<string | null>(null)
  const task = useTask<Summary>()
  const exporting = useTask<{ saved: boolean }>()
  const projectRef = useRef(project)
  const importing = useRef(false)
  const busy = task.running || exporting.running || preparing
  const report = task.result

  useEffect(() => { projectRef.current = project }, [project])
  useEffect(() => {
    if (!task.result || !importing.current) return
    const result = task.result
    const timer = setTimeout(() => {
      importing.current = false
      const next: Project = { ...projectRef.current, mode: 'json', text: result.modelText, seed: result.seed }
      projectRef.current = next
      setProject(next)
    }, 0)
    return () => clearTimeout(timer)
  }, [task.result, setProject])

  const update = (patch: Partial<Project>) => {
    const next = { ...projectRef.current, ...patch }
    projectRef.current = next
    setProject(next)
    task.reset()
    exporting.reset()
    setInputError(null)
  }
  const input = () => projectRef.current.mode === 'json' ? { text: projectRef.current.text } : { draft: projectRef.current.draft }
  const factor = (index: number, patch: Partial<DraftFactor>) => {
    const draft = projectRef.current.draft
    update({ draft: { ...draft, factors: draft.factors.map((f, i) => i === index ? { ...f, ...patch } : f) } })
  }
  const changeMode = async (mode: Project['mode']) => {
    if (busy || mode === projectRef.current.mode) return
    setPreparing(true)
    try {
      const prepared = await callPlugin<Prepared>(manifest.id, 'prepare', input())
      update({ mode, text: prepared.modelText, draft: { factors: prepared.factors, forbiddenText: prepared.forbiddenText } })
    } catch (error) { setInputError(errorMessage(error)) }
    finally { setPreparing(false) }
  }
  const start = () => {
    importing.current = false
    setInputError(null)
    exporting.reset()
    void task.start('generate', { ...input(), seed: projectRef.current.seed, limits: projectRef.current.limits })
  }
  const openFile = async () => {
    try {
      const path = await host.dialog.openFile([{ name: t('input.jsonFile'), extensions: ['json'] }])
      if (!path) return
      importing.current = true
      exporting.reset()
      setInputError(null)
      await task.start('generate', { path, seed: projectRef.current.seed, limits: projectRef.current.limits })
    } catch (error) { toast.error(errorMessage(error)) }
  }
  const save = async (format: 'csv' | 'json') => {
    if (!report) return
    const snapshot = report
    try {
      const partial = snapshot.status !== 'complete'
      const path = await host.dialog.saveFile(`pairwise${partial ? '.partial' : ''}.${format}`, [{ name: format.toUpperCase(), extensions: [format] }])
      if (path) await exporting.start('export', { handle: snapshot.handle, path, format, allowPartial: partial })
    } catch (error) { toast.error(errorMessage(error)) }
  }
  const cancel = async () => {
    try { await task.cancel() } catch (error) { toast.error(errorMessage(error)) }
  }
  const useReportModel = () => {
    if (report && !busy) update({ mode: 'json', text: report.modelText, seed: report.seed })
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(330px,0.8fr)_minmax(0,1.2fr)] gap-3">
      <Panel icon={<Boxes />} title={t('input.title')} extra={<Badge variant="neutral">{t('input.autosaved')}</Badge>} bodyClassName="flex h-full min-h-0 flex-col gap-3 p-3"
        actions={<><Tooltip content={t('input.open')}><Button size="icon-sm" variant="ghost" disabled={busy} onClick={() => void openFile()} aria-label={t('input.open')}><FolderOpen /></Button></Tooltip><Tooltip content={t('input.example')}><Button size="icon-sm" variant="ghost" disabled={busy} onClick={() => update(initialProject())} aria-label={t('input.example')}><RotateCcw /></Button></Tooltip></>}
        footer={task.running ? <Button size="sm" onClick={() => void cancel()}><Square />{t('input.cancel')}</Button> : <Button size="sm" variant="primary" disabled={busy} onClick={start}><CheckCheck />{t('input.generate')}</Button>}>
        <div className={busy ? 'pointer-events-none opacity-60' : ''} aria-busy={preparing}>
          <SegmentedControl size="sm" value={project.mode} onValueChange={(mode) => void changeMode(mode)} aria-label={t('input.mode')} options={[{ value: 'form', label: t('input.form') }, { value: 'json', label: t('input.json') }]} />
        </div>
        {inputError && <p role="alert" className="text-xs text-danger">{inputError}</p>}
        {project.mode === 'form' ? (
          <div className="min-h-0 flex-1 space-y-3 overflow-auto pr-1">
            <p className="text-xs leading-relaxed text-fg-muted">{t('input.valuesHint')}</p>
            <div className="space-y-2">
              {project.draft.factors.map((f, index) => (
                <div key={index} className="space-y-2 rounded-control border border-border bg-surface-2 p-2.5">
                  <div className="flex items-center gap-2">
                    <Input size="sm" value={f.name} disabled={busy} onChange={(event) => factor(index, { name: event.target.value })} aria-label={t('input.factorName')} placeholder={t('input.factorName')} wrapperClassName="min-w-0 flex-1" />
                    <Tooltip content={t('input.removeFactor')}><Button size="icon-sm" variant="ghost" disabled={busy || project.draft.factors.length <= 2} aria-label={t('input.removeFactor')} onClick={() => update({ draft: { ...project.draft, factors: project.draft.factors.filter((_, i) => i !== index) } })}><Trash2 /></Button></Tooltip>
                  </div>
                  <Input size="sm" value={f.valuesText} disabled={busy} onChange={(event) => factor(index, { valuesText: event.target.value })} aria-label={t('input.valuesFor', { name: f.name })} className="font-mono" />
                </div>
              ))}
              <Button size="sm" variant="ghost" disabled={busy || project.draft.factors.length >= 12} onClick={() => update({ draft: { ...project.draft, factors: [...project.draft.factors, { name: '', valuesText: '[true, false]' }] } })}><Plus />{t('input.addFactor')}</Button>
            </div>
            <div className="space-y-2">
              <h4 className="text-xs font-medium text-fg">{t('input.constraints')}</h4>
              <p className="text-[11px] leading-relaxed text-fg-muted">{t('input.constraintHint')}</p>
              <div className="h-40 overflow-hidden rounded-control border border-border"><CodeEditor value={project.draft.forbiddenText} onChange={(forbiddenText) => update({ draft: { ...project.draft, forbiddenText } })} readOnly={busy} language="json" aria-label={t('input.constraints')} /></div>
            </div>
          </div>
        ) : <div className="min-h-0 flex-1 overflow-hidden rounded-control border border-border"><CodeEditor value={project.text} onChange={(text) => update({ text })} readOnly={busy} language="json" aria-label={t('input.json')} /></div>}
        <div className="shrink-0 space-y-2 border-t border-border pt-3">
          <label className="flex items-center gap-2 text-xs text-fg-muted"><span className="w-20 shrink-0">{t('input.seed')}</span><Input size="sm" disabled={busy} value={project.seed} maxLength={80} onChange={(event) => update({ seed: event.target.value })} aria-label={t('input.seed')} wrapperClassName="min-w-0 flex-1" className="font-mono" /></label>
          {([
            ['maxCases', [25, 100, 500, 1000]], ['maxCombinations', [1000, 10000, 50000]], ['budgetMs', [500, 1000, 5000]],
          ] as const).map(([key, options]) => <div key={key} className="flex items-center justify-between gap-2 text-xs text-fg-muted"><span>{t(`input.${key}`)}</span><Select size="sm" value={project.limits[key]} disabled={busy} options={options.map((value) => ({ value, label: t(key === 'budgetMs' ? 'input.milliseconds' : 'input.limitNumber', { value }) }))} onValueChange={(value) => update({ limits: { ...project.limits, [key]: value } })} aria-label={t(`input.${key}`)} /></div>)}
          <p className="text-[11px] text-fg-subtle">{t('input.limitsHint')}</p>
        </div>
      </Panel>
      <Panel icon={<CheckCheck />} title={t('result.title')} bodyClassName="flex h-full min-h-0 flex-col gap-3 p-3"
        extra={report && <Badge variant={report.status === 'complete' ? 'success' : 'warning'}>{t(`status.${report.status}`)}</Badge>}
        actions={<><Tooltip content={t('export.csv')}><Button size="icon-sm" variant="ghost" disabled={!report || report.status !== 'complete' || busy} aria-label={t('export.csv')} onClick={() => void save('csv')}><Download /></Button></Tooltip><Tooltip content={t(report?.status === 'complete' ? 'export.json' : 'export.partial')}><Button size="icon-sm" variant="ghost" disabled={!report || busy} aria-label={t(report?.status === 'complete' ? 'export.json' : 'export.partial')} onClick={() => void save('json')}><FileJson /></Button></Tooltip></>}
        footer={report && <span className="truncate font-mono text-[10px] text-fg-subtle">{t('result.algorithm', { version: report.algorithm, seed: report.seed })}</span>}>
        {task.error && <p role="alert" className="text-xs text-danger">{errorMessage(task.error)}</p>}
        {exporting.error && <p role="alert" className="text-xs text-danger">{errorMessage(exporting.error)}</p>}
        {exporting.status === 'succeeded' && <p role="status" className="text-xs text-success">{t('export.saved')}</p>}
        {task.running && <div className="space-y-2"><p role="status" className="flex items-center gap-2 text-xs text-fg"><Spinner className="size-3.5" />{t(`stages.${task.stage ?? 'parse'}`)}</p>{task.progress && <Progress value={task.progress.total ? task.progress.done / task.progress.total * 100 : 0} />}</div>}
        {report ? <div className="min-h-0 flex-1"><Results key={report.handle} report={report} onUseModel={useReportModel} /></div> : !task.running && !task.error && <div className="flex min-h-0 flex-1 items-center justify-center"><Empty icon={<Boxes />} title={t('result.empty')} description={t('result.emptyHint')} /></div>}
        {(task.logs.length > 0 || exporting.running) && <details open={task.running} className="shrink-0 rounded-control border border-border"><summary className="cursor-pointer px-2.5 py-1.5 text-xs text-fg-muted">{t('result.logs')}</summary><LogViewer entries={exporting.running ? exporting.logs : task.logs} className="h-28" emptyText={t('result.noLogs')} jumpLabel={t('result.latest')} /></details>}
      </Panel>
    </div>
  )
}
