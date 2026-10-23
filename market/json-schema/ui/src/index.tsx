import { useEffect, useRef, useState } from 'react'
import { Badge, Button, CodeEditor, DropdownMenu, Empty, Panel, Progress, Select, Switch, toast, type CodeEditorHandle } from '@toolforge/ui'
import { CopyButton, host, useLaunchInput, usePlugin, usePluginState, useTask } from '@toolforge/plugin-ui-sdk'
import { BookOpen, Braces, CheckCheck, CircleAlert, Download, FileJson, FolderOpen, Play, Sparkles, Square, X } from 'lucide-react'
import { sample } from './samples'
import type { Diagnostic, Draft, InferResult, InferSplit, Report, Submission } from './types'

function JsonSchema() {
  const { t, call, errorMessage } = usePlugin()
  const [draft, setDraft] = usePluginState<Draft>('draft', '2020-12')
  const [validateFormats, setValidateFormats] = usePluginState('validateFormats', false)
  const [instance, setInstance] = useState(() => sample('invalid', '2020-12').instance)
  const [schema, setSchema] = useState(() => sample('invalid', '2020-12').schema)
  const [submitted, setSubmitted] = useState<Submission | null>(null)
  const [cancelRequested, setCancelRequested] = useState(false)
  const [cancelError, setCancelError] = useState<string | null>(null)
  const [selected, setSelected] = useState<number | null>(null)
  const instanceEditor = useRef<CodeEditorHandle>(null)
  const schemaEditor = useRef<CodeEditorHandle>(null)
  const task = useTask<Report>()
  const fileSequence = useRef({ instance: 0, schema: 0 })

  useEffect(() => () => { fileSequence.current.instance++; fileSequence.current.schema++ }, [])
  const change = (target: Diagnostic['target'], value: string) => {
    fileSequence.current[target]++
    setSelected(null)
    if (target === 'instance') setInstance(value)
    else setSchema(value)
  }
  const openFile = async (target: Diagnostic['target']) => {
    const sequence = ++fileSequence.current[target]
    try {
      const path = await host.dialog.openFile([{ name: t('input.fileFilter'), extensions: ['json', 'schema', 'txt'] }])
      if (!path || sequence !== fileSequence.current[target]) return
      const text = await call<string>('read_file', { path })
      if (sequence === fileSequence.current[target]) change(target, text)
    } catch (error) { if (sequence === fileSequence.current[target]) toast.error(errorMessage(error)) }
  }
  const saveReport = async () => {
    if (!task.result) return
    const text = JSON.stringify(task.result, null, 2)
    try {
      const path = await host.dialog.saveFile('schema-report.json', [{ name: t('input.fileFilter'), extensions: ['json'] }])
      if (path) { await host.fs.writeText(path, text); toast.success(t('common:saved')) }
    } catch (error) { toast.error(errorMessage(error)) }
  }

  const infer = async (split: InferSplit) => {
    try {
      const result = await call<InferResult>('infer', { input: instance, split, draft, enums: true })
      change('schema', result.schema)
      toast.success(t('infer.done', { count: result.samples }))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  useLaunchInput((text, label) => {
    change('schema', text)
    setDraft(label === 'schema7' ? '7' : '2020-12')
  })

  const stale = submitted !== null && (submitted.instance !== instance || submitted.schema !== schema || submitted.draft !== draft || submitted.validateFormats !== validateFormats)
  const report = task.result
  const diagnostics = report?.diagnostics ?? []
  const syntaxLine = (target: Diagnostic['target']) => !stale ? diagnostics.find((d) => d.target === target && d.line !== null)?.line : null
  const active = selected !== null ? diagnostics[selected] : null

  const start = () => {
    const args = { instance, schema, draft, validateFormats }
    setSubmitted(args)
    setSelected(null)
    setCancelRequested(false)
    setCancelError(null)
    void task.start('validate', args)
  }
  const cancel = async () => {
    setCancelRequested(true)
    try { await task.cancel() } catch (error) { setCancelRequested(false); setCancelError(errorMessage(error)) }
  }
  const choose = (key: 'valid' | 'invalid' | 'refs' | 'tuple') => {
    const next = sample(key, draft)
    change('instance', next.instance)
    change('schema', next.schema)
    setSelected(null)
  }
  const selectDiagnostic = (diagnostic: Diagnostic, index: number) => {
    setSelected(index)
    if (!stale && diagnostic.line) {
      const editor = diagnostic.target === 'instance' ? instanceEditor : schemaEditor
      editor.current?.gotoLine(diagnostic.line, diagnostic.column ?? 1)
    }
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <Panel className="shrink-0" title={t('settings.title')} icon={<CheckCheck />} bodyClassName="space-y-2.5 p-3">
        <div className="flex flex-wrap items-center gap-3">
          <Select<Draft> value={draft} onValueChange={setDraft} aria-label={t('settings.draft')} options={[
            { value: '2020-12', label: t('drafts.2020-12') },
            { value: '7', label: t('drafts.7') },
          ]} />
          <label className="flex items-center gap-2 text-xs text-fg-muted">
            <Switch size="sm" checked={validateFormats} onCheckedChange={setValidateFormats} aria-label={t('settings.formats')} />
            {t('settings.formats')}
          </label>
          <DropdownMenu trigger={<Button size="sm"><BookOpen />{t('samples.title')}</Button>} items={[
            { key: 'valid', label: t('samples.valid'), onSelect: () => choose('valid') },
            { key: 'invalid', label: t('samples.invalid'), onSelect: () => choose('invalid') },
            { key: 'refs', label: t('samples.refs'), onSelect: () => choose('refs') },
            { key: 'tuple', label: t('samples.tuple'), onSelect: () => choose('tuple') },
          ]} />
          <DropdownMenu placement="bottom-start" trigger={<Button size="sm" disabled={!instance.trim()}><Sparkles />{t('infer.title')}</Button>} items={[
            { key: 'document', label: t('infer.document'), onSelect: () => void infer('document') },
            { key: 'array', label: t('infer.array'), onSelect: () => void infer('array') },
            { key: 'lines', label: t('infer.lines'), onSelect: () => void infer('lines') },
          ]} />
          <span className="ml-auto flex items-center gap-2">
            {task.running ? <Button size="sm" onClick={cancel} disabled={cancelRequested}><Square />{t(cancelRequested ? 'actions.cancelling' : 'actions.cancel')}</Button> : (
              <Button size="sm" variant="primary" onClick={start} disabled={!instance.trim() || !schema.trim()}><Play />{t('actions.validate')}</Button>
            )}
          </span>
        </div>
        <p className="text-[11px] text-fg-subtle">{t('settings.hint')}</p>
        {task.running && <div className="flex items-center gap-3"><Progress className="max-w-48 flex-1" value={null} /><span role="status" className="text-xs text-fg-muted">{t(cancelRequested ? 'status.cancelHint' : `stages.${task.stage ?? 'schema.parse'}`)}</span></div>}
      </Panel>

      <div className="grid min-h-0 flex-1 grid-cols-2 gap-3">
        <Panel title={t('input.instance')} icon={<FileJson />} bodyClassName="p-0" actions={<>
          <CopyButton text={instance} />
          <Button size="icon-sm" variant="ghost" aria-label={t('input.openInstance')} onClick={() => void openFile('instance')}><FolderOpen /></Button>
          <Button size="icon-sm" variant="ghost" aria-label={t('input.clearInstance')} onClick={() => change('instance', '')}><X /></Button>
        </>}>
          <CodeEditor ref={instanceEditor} value={instance} onChange={(value) => change('instance', value)} language="json" errorLine={syntaxLine('instance')} aria-label={t('input.instance')} placeholder={t('input.instanceHint')} />
        </Panel>
        <Panel title={t('input.schema')} icon={<Braces />} bodyClassName="p-0" actions={<>
          <CopyButton text={schema} />
          <Button size="icon-sm" variant="ghost" aria-label={t('input.openSchema')} onClick={() => void openFile('schema')}><FolderOpen /></Button>
          <Button size="icon-sm" variant="ghost" aria-label={t('input.clearSchema')} onClick={() => change('schema', '')}><X /></Button>
        </>}>
          <CodeEditor ref={schemaEditor} value={schema} onChange={(value) => change('schema', value)} language="json" errorLine={syntaxLine('schema')} aria-label={t('input.schema')} placeholder={t('input.schemaHint')} />
        </Panel>
      </div>

      <Panel className="max-h-[45%] min-h-44 shrink-0" title={t('result.title')} icon={<CheckCheck />} bodyClassName="overflow-auto p-3" extra={report && <>
        <Badge variant={report.valid ? 'success' : 'danger'}>{t(report.valid ? 'result.valid' : 'result.invalid')}</Badge>
        <span className="text-[11px] text-fg-subtle">{t('result.elapsed', { ms: report.elapsedMs })}</span>
      </>} actions={report && <><CopyButton text={JSON.stringify(report, null, 2)} label={t('result.copy')} /><Button size="icon-sm" variant="ghost" aria-label={t('result.save')} onClick={() => void saveReport()}><Download /></Button></>} footer={stale ? <span className="text-warning">{t('result.stale')}</span> : report?.truncated ? <span className="text-warning">{t('result.truncated')}</span> : undefined}>
        {task.error || cancelError ? <p role="alert" className="text-xs text-danger">{cancelError ?? errorMessage(task.error)}</p> : task.status === 'cancelled' ? <p role="status" className="text-xs text-fg-muted">{t('status.cancelled')}</p> : !report ? (
          <Empty icon={<CheckCheck />} title={t('result.empty')} description={t('result.emptyHint')} />
        ) : report.valid ? <div role="status" className="flex items-center gap-2 text-sm text-success"><CheckCheck className="size-5" />{t('result.validHint')}</div> : <>
          <div className="mb-2 flex items-center gap-2 text-xs text-fg-muted"><CircleAlert className="size-4 text-danger" />{t('result.count', { count: diagnostics.length })}</div>
          <ol className="space-y-2">
            {diagnostics.map((diagnostic, index) => <li key={index}>
              <button type="button" onClick={() => selectDiagnostic(diagnostic, index)} aria-pressed={selected === index} className={`w-full rounded-control border p-2.5 text-left focus-visible:outline-2 focus-visible:outline-primary ${selected === index ? 'border-primary bg-primary-soft' : 'border-border bg-surface-2 hover:border-primary'}`}>
                <span className="mb-1 flex flex-wrap items-center gap-2 text-xs"><Badge variant={diagnostic.target === 'schema' ? 'warning' : 'danger'}>{t(`input.${diagnostic.target}`)}</Badge><span className="font-medium text-fg">{t(`diagnostics.${diagnostic.code}`)}</span>{diagnostic.keyword && <code className="text-fg-muted">{diagnostic.keyword}</code>}{diagnostic.line && <span className="text-fg-subtle">{t('result.location', { line: diagnostic.line, column: diagnostic.column })}</span>}</span>
                <span className="block break-all text-xs text-fg-muted">{diagnostic.detail}</span>
                <span className="mt-1 flex flex-wrap gap-x-4 gap-y-1 text-[11px] text-fg-subtle"><span>{t(diagnostic.target === 'schema' ? 'result.documentPath' : 'result.instancePath')} <code className="text-fg-muted">{diagnostic.instancePath || t('result.root')}</code></span><span>{t(diagnostic.target === 'schema' ? 'result.metaPath' : 'result.schemaPath')} <code className="text-fg-muted">{diagnostic.schemaPath || t('result.root')}</code></span></span>
              </button>
            </li>)}
          </ol>
          {active && !active.line && <p className="mt-2 text-[11px] text-fg-subtle">{t('result.pointerHint')}</p>}
        </>}
      </Panel>
    </div>
  )
}

export default JsonSchema
