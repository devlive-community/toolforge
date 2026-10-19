import { useState } from 'react'
import { Badge, Button, Empty, Panel, SegmentedControl, Tooltip, toast } from '@toolforge/ui'
import { callPlugin, host, usePlugin, usePluginState, useTask } from '@toolforge/plugin-ui-sdk'
import { CircleStop, Copy, FileDown, FilePlus2, GitCompareArrows, TriangleAlert } from 'lucide-react'
import type { Exported, Finding, Report, Severity, SideInput } from './types'

interface UiState {
  severityFilter: 'all' | Severity
}

const initialUiState = (): UiState => ({ severityFilter: 'all' })

const SEVERITY_ORDER: Severity[] = ['breaking', 'conditional', 'info']

/** i18n 里 findings 按规则 id 翻译；导出文本使用后端英文内容 */
function ruleLabel(t: (key: string, params?: Record<string, unknown>) => string, finding: Finding): string {
  return t(`rules.${finding.rule}`, { defaultValue: finding.rule })
}

export default function ProtobufDiff() {
  const { t, errorMessage, manifest } = usePlugin()
  const [ui, setUi] = usePluginState<UiState>('ui', initialUiState())
  const [oldInput, setOldInput] = useState<SideInput>({})
  const [newInput, setNewInput] = useState<SideInput>({})
  const [oldSource, setOldSource] = useState<'file' | 'base64'>('file')
  const [newSource, setNewSource] = useState<'file' | 'base64'>('file')
  const task = useTask<Report>()
  const report = task.result
  const [expanded, setExpanded] = useState<string | null>(null)

  const pickFile = async (side: 'old' | 'new') => {
    try {
      const path = await host.dialog.openFile([{ name: t('input.fileFilter'), extensions: ['bin', 'pb', 'desc', 'protoset', 'binpb'] }])
      if (!path) return
      if (side === 'old') {
        setOldSource('file')
        setOldInput({ path })
      } else {
        setNewSource('file')
        setNewInput({ path })
      }
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const start = () => {
    const side = (input: SideInput, source: 'file' | 'base64'): SideInput =>
      source === 'file' ? (input.path ? { path: input.path } : {}) : input.text?.trim() ? { text: input.text.trim() } : {}
    void task.start('diff', {
      oldPath: side(oldInput, oldSource).path,
      oldText: side(oldInput, oldSource).text,
      newPath: side(newInput, newSource).path,
      newText: side(newInput, newSource).text,
    })
  }

  const canStart = (oldSource === 'file' ? !!oldInput.path : !!oldInput.text?.trim()) && (newSource === 'file' ? !!newInput.path : !!newInput.text?.trim())

  const exportReport = async (format: 'json' | 'markdown' | 'text', save: boolean) => {
    if (!report) return
    try {
      const out = await callPlugin<Exported>(manifest.id, 'export', { format, report })
      if (save) {
        const path = await host.dialog.saveFile(`protobuf-report.${out.ext}`, [{ name: out.ext.toUpperCase(), extensions: [out.ext] }])
        if (!path) return
        await host.fs.writeText(path, out.content)
      } else {
        await host.clipboard.writeText(out.content)
      }
      toast.success(t('common:saved'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const visible = (report?.findings ?? []).filter((finding) => ui.severityFilter === 'all' || finding.severity === ui.severityFilter)
  const countOf = (severity: Severity) => report?.findings.filter((finding) => finding.severity === severity).length ?? 0

  const inputPanel = (side: 'old' | 'new', source: 'file' | 'base64', input: SideInput, onSource: (value: 'file' | 'base64') => void, onInput: (value: SideInput) => void) => {
    const label = side === 'old' ? t('input.old') : t('input.new')
    return (
      <Panel
        icon={<FilePlus2 />}
        title={label}
        bodyClassName="space-y-2 p-3"
        actions={
          <SegmentedControl
            value={source}
            onValueChange={(value) => {
              onSource(value)
              // 切换来源时清掉另一来源的残留
              onInput(value === 'file' ? { path: input.path } : { text: input.text })
            }}
            size="sm"
            aria-label={t('input.source')}
            options={[
              { value: 'file', label: t('input.sourceFile') },
              { value: 'base64', label: 'Base64' },
            ]}
          />
        }
      >
        {source === 'file' ? (
          <div className="flex items-center gap-2">
            <Button size="sm" variant="ghost" onClick={() => void pickFile(side)}>
              <FilePlus2 />
              {t('input.chooseFile')}
            </Button>
            <span className="truncate font-mono text-xs text-fg-muted">{input.path || t('input.noFile')}</span>
          </div>
        ) : (
          <textarea
            value={input.text ?? ''}
            onChange={(event) => onInput({ text: event.target.value })}
            placeholder={t('input.base64Placeholder')}
            aria-label={t('input.base64')}
            rows={4}
            className="w-full resize-none rounded-md border border-border-subtle bg-transparent p-2 font-mono text-xs outline-none focus:border-primary-500"
          />
        )}
      </Panel>
    )
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(360px,0.85fr)_minmax(0,1.15fr)] gap-3">
      <div className="flex min-h-0 flex-col gap-3 overflow-y-auto pb-1">
        {inputPanel('old', oldSource, oldInput, setOldSource, setOldInput)}
        {inputPanel('new', newSource, newInput, setNewSource, setNewInput)}
        <Panel icon={<GitCompareArrows />} title={t('input.runPanel')} bodyClassName="space-y-2 p-3">
          <div className="flex items-center gap-2">
            {task.running ? (
              <Button onClick={() => void task.cancel()}>
                <CircleStop />
                {t('input.cancel')}
              </Button>
            ) : (
              <Button onClick={start} disabled={!canStart}>
                <GitCompareArrows />
                {t('input.run')}
              </Button>
            )}
            {task.running && task.stage && <span className="text-xs text-fg-subtle">{t(`input.stages.${task.stage}`, { defaultValue: task.stage })}</span>}
          </div>
          <p className="text-xs text-fg-subtle">{t('input.hint')}</p>
        </Panel>
      </div>

      <Panel
        icon={<GitCompareArrows />}
        title={t('results.title')}
        bodyClassName="flex min-h-0 flex-col gap-2 p-3"
        actions={
          report && (
            <div className="flex items-center gap-1">
              <Tooltip content={t('results.copy')}>
                <Button size="icon-sm" variant="ghost" aria-label={t('results.copy')} onClick={() => void exportReport('text', false)}>
                  <Copy />
                </Button>
              </Tooltip>
              <Tooltip content={t('results.saveMarkdown')}>
                <Button size="icon-sm" variant="ghost" aria-label={t('results.saveMarkdown')} onClick={() => void exportReport('markdown', true)}>
                  <FileDown />
                </Button>
              </Tooltip>
            </div>
          )
        }
      >
        <div className="min-h-0 flex-1 space-y-2 overflow-y-auto">
          {task.error && <p className="text-xs text-fg-danger">{errorMessage(task.error)}</p>}
          {task.status === 'cancelled' && <p className="text-xs text-fg-muted">{t('results.cancelled')}</p>}
          {!report && !task.error && task.status !== 'cancelled' && <Empty title={t('results.empty')} />}
          {report && (
            <>
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant="neutral">{t('results.rulesVersion', { version: report.rulesVersion })}</Badge>
                {SEVERITY_ORDER.map((severity) => (
                  <Badge
                    key={severity}
                    variant={severity === 'breaking' ? 'danger' : severity === 'conditional' ? 'warning' : 'neutral'}
                  >
                    {t(`results.${severity}`, { count: countOf(severity) })}
                  </Badge>
                ))}
              </div>
              {report.unsupported.length > 0 && (
                <div className="flex items-start gap-2 rounded-lg border border-amber-500/40 bg-amber-500/10 p-2">
                  <TriangleAlert className="mt-0.5 size-3.5 shrink-0 text-amber-600 dark:text-amber-400" />
                  <div className="space-y-0.5 text-xs text-amber-600 dark:text-amber-400">
                    <p>{t('results.unsupportedTitle')}</p>
                    {report.unsupported.map((file) => (
                      <p key={file.name} className="font-mono">
                        {file.name} · {file.syntax}
                      </p>
                    ))}
                  </div>
                </div>
              )}
              <div className="flex items-center gap-2">
                <SegmentedControl
                  value={ui.severityFilter}
                  onValueChange={(value) => setUi({ ...ui, severityFilter: value })}
                  size="sm"
                  aria-label={t('results.filter')}
                  options={[
                    { value: 'all', label: t('results.filterAll') },
                    { value: 'breaking', label: t('results.filterBreaking') },
                    { value: 'conditional', label: t('results.filterConditional') },
                    { value: 'info', label: t('results.filterInfo') },
                  ]}
                />
              </div>
              {visible.length === 0 ? (
                <Empty title={t('results.noFindings')} />
              ) : (
                visible.map((finding) => {
                  const key = `${finding.rule}:${finding.path}:${finding.oldDesc}→${finding.newDesc}`
                  const open = expanded === key
                  return (
                    <button
                      key={key}
                      type="button"
                      onClick={() => setExpanded(open ? null : key)}
                      className="w-full rounded-lg border border-border-subtle p-2 text-left transition-colors hover:bg-bg-subtle"
                    >
                      <div className="flex items-center justify-between gap-2">
                        <p className="truncate font-mono text-xs">{finding.path}</p>
                        <Badge variant={finding.severity === 'breaking' ? 'danger' : finding.severity === 'conditional' ? 'warning' : 'neutral'}>
                          {t(`results.${finding.severity}`)}
                        </Badge>
                      </div>
                      <p className="mt-1 text-xs text-fg-muted">
                        {ruleLabel(t, finding)}
                        {finding.oldDesc && finding.newDesc && t('results.changeLine', { old: finding.oldDesc, new: finding.newDesc })}
                      </p>
                      {open && (
                        <div className="mt-1 space-y-0.5 text-xs text-fg-subtle">
                          {finding.location && <p className="font-mono">{finding.location}</p>}
                          {t(`rules.${finding.rule}.detail`, { defaultValue: '' }) && (
                            <p>{t(`rules.${finding.rule}.detail`, { defaultValue: '' })}</p>
                          )}
                          <p className="font-mono">{t('results.valuesLine', { old: finding.oldDesc || '—', new: finding.newDesc || '—' })}</p>
                        </div>
                      )}
                    </button>
                  )
                })
              )}
              <p className="text-xs text-fg-subtle">{t('results.notes')}</p>
            </>
          )}
        </div>
      </Panel>
    </div>
  )
}
