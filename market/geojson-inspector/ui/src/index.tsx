import { useRef, useState } from 'react'
import { Badge, Button, CodeEditor, DropdownMenu, Empty, Panel, Spinner, Tooltip, toast } from '@toolforge/ui'
import { callPlugin, host, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { FileJson, FolderOpen, ListChecks, Play, ShieldCheck, Square, X } from 'lucide-react'
import { SAMPLES } from './samples'
import type { Diag, Exported, Report } from './types'

const LEVEL_VARIANT = { error: 'danger', warning: 'warning', hint: 'info' } as const
const OVERALL_VARIANT = { passed: 'success', warnings: 'warning', invalid: 'danger', incomplete: 'neutral' } as const

function Location({ diag }: { diag: Diag }) {
  const { t } = usePlugin()
  const pointer = diag.pointer === '' ? t('result.notFound') : diag.pointer
  return (
    <span className="flex flex-wrap items-center gap-x-2 gap-y-0.5 text-[11px] text-fg-subtle">
      <code className="font-mono" data-selectable>{pointer}</code>
      {diag.line !== undefined ? (
        <span>{t('result.lineAt', { line: diag.line, column: diag.column ?? 0 })}</span>
      ) : (
        <span>{t('result.noLine')}</span>
      )}
    </span>
  )
}

export default function GeoJsonInspector() {
  const { t, manifest, errorMessage } = usePlugin()
  const [text, setText] = useState(() => SAMPLES.valid)
  const [cancelRequested, setCancelRequested] = useState(false)
  const task = useTask<Report>()
  const fileSequence = useRef(0)

  const report = task.result
  const openFile = async () => {
    const sequence = ++fileSequence.current
    try {
      const path = await host.dialog.openFile([{ name: t('input.fileFilter'), extensions: ['geojson', 'json', 'txt'] }])
      if (!path || sequence !== fileSequence.current) return
      const content = await callPlugin<string>(manifest.id, 'read_file', { path })
      if (sequence === fileSequence.current) setText(content)
    } catch (reason) {
      if (sequence === fileSequence.current) toast.error(errorMessage(reason))
    }
  }

  const start = () => {
    setCancelRequested(false)
    void task.start('inspect', { text })
  }
  const cancel = async () => {
    setCancelRequested(true)
    try { await task.cancel() } catch (reason) { setCancelRequested(false); toast.error(errorMessage(reason)) }
  }

  const exportReport = async (format: 'markdown' | 'json', save: boolean) => {
    try {
      const out = await callPlugin<Exported>(manifest.id, 'export', { format })
      if (save) {
        const path = await host.dialog.saveFile(`geojson-report.${out.ext}`, [
          { name: out.ext.toUpperCase(), extensions: [out.ext] },
        ])
        if (!path) return
        await host.fs.writeText(path, out.content)
        toast.success(t('common:saved'))
      } else {
        await host.clipboard.writeText(out.content)
        toast.success(t('common:saved'))
      }
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(380px,0.95fr)_minmax(0,1.05fr)] gap-3">
      <Panel
        icon={<FileJson />}
        title={t('input.text')}
        bodyClassName="h-full min-h-0 p-3"
        actions={
          <>
            <DropdownMenu trigger={<Button size="sm">{t('input.samples')}</Button>} items={[
              { key: 'valid', label: t('input.samples.valid'), onSelect: () => setText(SAMPLES.valid) },
              { key: 'seven', label: t('input.samples.seven'), onSelect: () => setText(SAMPLES.seven) },
              { key: 'elevation', label: t('input.samples.elevation'), onSelect: () => setText(SAMPLES.elevation) },
              { key: 'swapped', label: t('input.samples.swapped'), onSelect: () => setText(SAMPLES.swapped) },
              { key: 'unclosed', label: t('input.samples.unclosed'), onSelect: () => setText(SAMPLES.unclosed) },
              { key: 'antimeridian', label: t('input.samples.antimeridian'), onSelect: () => setText(SAMPLES.antimeridian) },
              { key: 'legacyCrs', label: t('input.samples.legacyCrs'), onSelect: () => setText(SAMPLES.legacyCrs) },
              { key: 'emptyCollection', label: t('input.samples.emptyCollection'), onSelect: () => setText(SAMPLES.emptyCollection) },
            ]} />
            <Tooltip content={t('input.open')}>
              <Button size="icon-sm" variant="ghost" aria-label={t('input.open')} onClick={() => void openFile()}>
                <FolderOpen />
              </Button>
            </Tooltip>
            <Tooltip content={t('input.clear')}>
              <Button size="icon-sm" variant="ghost" aria-label={t('input.clear')} onClick={() => setText('')}>
                <X />
              </Button>
            </Tooltip>
          </>
        }
        footer={
          task.running ? (
            <div className="flex w-full items-center gap-3">
              <Button size="sm" onClick={cancel} disabled={cancelRequested}>
                <Square />
                {t(cancelRequested ? 'input.cancelling' : 'input.cancel')}
              </Button>
              <span role="status" className="flex items-center gap-2 text-xs text-fg-muted">
                <Spinner className="size-3.5" />
                {task.stage ? t(`input.stages.${task.stage}`) : t('input.run')}
              </span>
            </div>
          ) : (
            <Button size="sm" variant="primary" onClick={start} disabled={!text.trim()}>
              <Play />
              {t('input.run')}
            </Button>
          )
        }
      >
        <CodeEditor
          value={text}
          onChange={setText}
          language="json"
          aria-label={t('input.text')}
          placeholder={t('input.empty')}
        />
      </Panel>

      <Panel
        icon={<ListChecks />}
        title={t('result.title')}
        bodyClassName="h-full min-h-0 space-y-3 overflow-y-auto p-3"
        extra={report && <Badge variant={OVERALL_VARIANT[report.overall]}>{t(`overall.${report.overall}`)}</Badge>}
        footer={
          report && (
            <div className="flex w-full items-center gap-2">
              <Button size="sm" variant="soft" onClick={() => void exportReport('markdown', false)}>
                {t('export.markdown')}
              </Button>
              <Button size="sm" variant="soft" onClick={() => void exportReport('json', false)}>
                {t('export.json')}
              </Button>
              <span className="ml-auto flex items-center gap-2">
                <Button size="sm" onClick={() => void exportReport('markdown', true)}>
                  {t('export.saveMd')}
                </Button>
                <Button size="sm" onClick={() => void exportReport('json', true)}>
                  {t('export.saveJson')}
                </Button>
              </span>
            </div>
          )
        }
      >
        {task.error && (
          <p className="text-xs text-danger" role="alert">{errorMessage(task.error)}</p>
        )}
        {task.status === 'cancelled' && (
          <p className="text-xs text-fg-muted" role="status">{t('errors.geojson.cancelled')}</p>
        )}
        {!report && !task.error && task.status !== 'cancelled' && (
          <div className="flex h-full items-center justify-center">
            <Empty icon={<ShieldCheck />} title={t('result.empty')} description={t('input.empty')} />
          </div>
        )}
        {report && (
          <>
            {report.incomplete && (
              <p className="rounded-control border border-warning bg-warning/10 p-2.5 text-xs text-warning" role="alert">
                {t('result.incomplete')}
              </p>
            )}
            <div className="flex flex-wrap items-center gap-2 text-xs text-fg-muted">
              <span className="rounded-control bg-active px-2 py-1">{t('result.stats.features')}: {report.stats.features}</span>
              <span className="rounded-control bg-active px-2 py-1">{t('result.stats.positions')}: {report.stats.positions}</span>
              <span className="rounded-control bg-active px-2 py-1">{t('result.level.error')}: {report.stats.errors}</span>
              <span className="rounded-control bg-active px-2 py-1">{t('result.level.warning')}: {report.stats.warnings}</span>
              <span className="rounded-control bg-active px-2 py-1">{t('result.level.hint')}: {report.stats.hints}</span>
            </div>
            {Object.keys(report.stats.types).length > 0 && (
              <div className="flex flex-wrap items-center gap-1.5 text-xs text-fg-muted">
                <span>{t('result.stats.types')}</span>
                {Object.entries(report.stats.types).map(([kind, count]) => (
                  <code key={kind} className="rounded-control bg-active px-1.5 py-0.5 font-mono text-fg">
                    {t('result.typeCount', { kind, count })}
                  </code>
                ))}
              </div>
            )}
            {report.diagnostics.length === 0 ? (
              <p className="flex items-center gap-2 text-sm text-success" role="status">
                <ShieldCheck className="size-5" />
                {t(`overall.${report.overall}`)}
              </p>
            ) : (
              <ol className="space-y-2">
                {report.diagnostics.map((diag, index) => (
                  <li key={index} className="rounded-control border border-border bg-surface-2 p-2.5">
                    <span className="mb-1 flex flex-wrap items-center gap-2 text-xs">
                      <Badge variant={LEVEL_VARIANT[diag.level]}>{t(`result.level.${diag.level}`)}</Badge>
                      <code className="font-mono text-fg-muted">{diag.code}</code>
                    </span>
                    <p className="text-xs text-fg">{errorMessage(diag)}</p>
                    <Location diag={diag} />
                  </li>
                ))}
              </ol>
            )}
          </>
        )}
      </Panel>
    </div>
  )
}
