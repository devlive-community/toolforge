import { useEffect, useMemo, useRef, useState } from 'react'
import {
  Badge,
  Button,
  CodeEditor,
  Empty,
  Input,
  Modal,
  NumberInput,
  Panel,
  Progress,
  SegmentedControl,
  Spinner,
  cn,
  toast,
} from '@toolforge/ui'
import { callPlugin, host, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { FileCode2, Frame, ListTree, MapPinned, TriangleAlert, Upload } from 'lucide-react'
import type { Diag, Exported, FrameOut, FrameStatus, Overview, Symbolication } from './types'

const SAMPLE = `Uncaught TypeError: x is not a function
    at doWork (http://localhost:3000/assets/app.min.js:1:9)
    at init (http://localhost:3000/assets/app.min.js:3:5)`

function basename(path: string | null | undefined): string {
  if (!path) return ''
  const trimmed = path.replace(/[/\\]+$/, '')
  const at = Math.max(trimmed.lastIndexOf('/'), trimmed.lastIndexOf('\\'))
  return at >= 0 ? trimmed.slice(at + 1) : trimmed
}

const STATUS_VARIANT: Record<FrameStatus, 'success' | 'danger' | 'warning' | 'neutral'> = {
  resolved: 'success',
  no_mapping: 'danger',
  unrelated: 'warning',
  unassociated: 'warning',
  no_column: 'warning',
  message: 'neutral',
}

function Chip({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <span className="inline-flex items-center gap-1.5 text-xs">
      <span className="text-fg-subtle">{label}</span>
      <span className={cn('text-fg', mono && 'font-mono')} data-selectable>
        {value}
      </span>
    </span>
  )
}

function StatusBadge({ status }: { status: FrameStatus }) {
  const { t } = usePlugin()
  return <Badge variant={STATUS_VARIANT[status]}>{t(`frames.status.${status}`)}</Badge>
}

function SourceContextView({ frame }: { frame: FrameOut }) {
  const { t } = usePlugin()
  const resolved = frame.resolved
  if (!resolved) return null
  return (
    <div className="space-y-2">
      <p className="text-xs font-medium text-fg-muted">{t('frames.context')}</p>
      {resolved.context ? (
        <div className="overflow-x-auto rounded-control border border-border bg-surface-2 py-1 font-mono text-xs leading-5">
          {resolved.context.lines.map((line, index) => {
            const target = index === resolved.context?.target
            return (
              <div key={line.line}>
                <div className={cn('flex gap-3 px-3', target && 'bg-primary-soft')}>
                  <span className="w-10 shrink-0 text-right text-fg-subtle">{line.line}</span>
                  <span className="whitespace-pre text-fg" data-selectable>
                    {line.text}
                  </span>
                </div>
                {target && (
                  <div className="flex gap-3 px-3 text-warning">
                    <span className="w-10 shrink-0" />
                    {/* 列号按 UTF-16 计，JS 的字符串索引天然一致 */}
                    <span className="whitespace-pre">{`${' '.repeat(Math.max(resolved.column - 1, 0))}^`}</span>
                  </div>
                )}
              </div>
            )
          })}
        </div>
      ) : (
        <p className="text-xs text-fg-subtle">{t('frames.noContent')}</p>
      )}
    </div>
  )
}

function FrameDetail({ frame }: { frame: FrameOut }) {
  const { t } = usePlugin()
  const resolved = frame.resolved
  return (
    <div className="space-y-3">
      <p className="text-xs font-medium text-fg-muted">{t('frames.detail')}</p>
      <div className="rounded-control border border-border bg-surface-2 px-3 py-2">
        <code className="font-mono text-xs break-all text-fg" data-selectable>
          {frame.raw}
        </code>
      </div>
      {resolved && (
        <div className="space-y-1 text-xs">
          <Chip
            label={t('frames.resolved')}
            value={`${resolved.source ?? t('frames.anonymous')}:${resolved.line}:${resolved.column}`}
            mono
          />
          {resolved.name && <Chip label={t('frames.name')} value={resolved.name} mono />}
          {resolved.section && (
            <Chip
              label={t('frames.section')}
              value={`${resolved.section.line}:${resolved.section.column}`}
              mono
            />
          )}
        </div>
      )}
      <SourceContextView frame={frame} />
    </div>
  )
}

export default function SourceMapSymbolicator() {
  const { t, manifest, errorMessage } = usePlugin()
  const loading = useTask<Overview>()
  const [pasted, setPasted] = useState<Overview | null>(null)
  const [assetDraft, setAssetDraft] = useState<{ handle: number; value: string } | null>(null)
  const [mode, setMode] = useState<'stack' | 'position'>('stack')
  const [stack, setStack] = useState(SAMPLE)
  const [position, setPosition] = useState({ line: 1, column: 1 })
  const [outcome, setOutcome] = useState<{ handle: number; data: Symbolication } | null>(null)
  const [selected, setSelected] = useState(0)
  const [busy, setBusy] = useState(false)
  const [failure, setFailure] = useState<Diag | null>(null)
  const [pasteOpen, setPasteOpen] = useState(false)
  const [pasteText, setPasteText] = useState('')
  const [dropping, setDropping] = useState(false)

  // 文件加载失败提示（粘贴路径在事件处理器里直接 toast）
  useEffect(() => {
    if (loading.status === 'failed' && loading.error) {
      toast.error(errorMessage(loading.error))
    }
  }, [loading.status]) // eslint-disable-line react-hooks/exhaustive-deps

  // 概览与关联产物：任务结果直接派生，新句柄自动重置手改的产物名与结果
  const taskOverview = loading.status === 'succeeded' ? loading.result : null
  const info = pasted ?? taskOverview
  const asset =
    assetDraft && info && assetDraft.handle === info.handle ? assetDraft.value : basename(info?.file)
  const result = outcome && info && outcome.handle === info.handle ? outcome.data : null

  const load = (path: string) => {
    setFailure(null)
    setPasted(null)
    void loading.start('load', { path })
  }

  const loadRef = useRef(load)
  useEffect(() => {
    loadRef.current = load
  })
  useEffect(() => {
    const off = host.onFileDrop({ drop: (paths) => paths[0] && loadRef.current(paths[0]), over: setDropping })
    return () => {
      void off.then((unlisten) => unlisten())
    }
  }, [])

  const openFile = async () => {
    const path = await host.dialog.openFile([{ name: t('open.filter'), extensions: ['map', 'json'] }])
    if (path) load(path)
  }

  const parsePaste = async () => {
    try {
      const overview = await callPlugin<Overview>(manifest.id, 'parse', { text: pasteText })
      setPasted(overview)
      setFailure(null)
      setPasteOpen(false)
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const symbolicate = async () => {
    if (!info) return
    setBusy(true)
    setFailure(null)
    try {
      const args =
        mode === 'stack'
          ? { handle: info.handle, stack, asset }
          : { handle: info.handle, asset, position: { line: position.line, column: position.column } }
      const data = await callPlugin<Symbolication>(manifest.id, 'symbolicate', args)
      setOutcome({ handle: info.handle, data })
      setSelected(0)
    } catch (reason) {
      setFailure(reason as Diag)
    } finally {
      setBusy(false)
    }
  }

  const exportReport = (format: 'markdown' | 'json') => {
    if (!info) return Promise.reject(new Error(t('export.empty')))
    return callPlugin<Exported>(manifest.id, 'export', { handle: info.handle, format })
  }

  const copyReport = async (format: 'markdown' | 'json') => {
    try {
      const out = await exportReport(format)
      await host.clipboard.writeText(out.content)
      toast.success(t('common:saved'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const saveReport = async (format: 'markdown' | 'json') => {
    try {
      const out = await exportReport(format)
      const path = await host.dialog.saveFile(`sourcemap-report.${out.ext}`, [
        { name: out.ext.toUpperCase(), extensions: [out.ext] },
      ])
      if (!path) return
      await host.fs.writeText(path, out.content)
      toast.success(t('common:saved'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const contentCovered = useMemo(() => {
    if (!info || info.sources.length === 0) return null
    return info.sources.filter((source) => source.hasContent).length
  }, [info])

  const selectedFrame = result && result.frames.length > 0 ? result.frames[Math.min(selected, result.frames.length - 1)] : null

  if (!info) {
    return (
      <>
        <div className="flex h-full flex-col items-center justify-center gap-3">
          <Empty icon={<MapPinned />} title={t('open.emptyTitle')} description={t('open.emptyHint')} />
          <div className="flex items-center gap-2">
            <Button onClick={() => void openFile()}>
              <Upload />
              {t('open.openFile')}
            </Button>
            <Button variant="soft" onClick={() => setPasteOpen(true)}>
              {t('open.paste')}
            </Button>
          </div>
          {dropping && <p className="text-xs text-fg-muted">{t('open.drop')}</p>}
        </div>
        <Modal open={pasteOpen} onOpenChange={setPasteOpen} size="lg" title={t('open.pasteTitle')}>
          <div className="space-y-3">
            <div className="h-64 overflow-hidden rounded-control border border-border">
              <CodeEditor
                value={pasteText}
                onChange={setPasteText}
                language="json"
                placeholder='{"version":3, …}'
                aria-label={t('open.pasteTitle')}
              />
            </div>
            <div className="flex justify-end gap-2">
              <Button onClick={() => void parsePaste()} disabled={pasteText.trim().length === 0}>
                {t('open.parse')}
              </Button>
            </div>
          </div>
        </Modal>
      </>
    )
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <section className="shrink-0 space-y-2 rounded-card border border-border bg-surface p-3 shadow-card">
        <div className="flex flex-wrap items-center gap-x-4 gap-y-1">
          <Badge variant="info">{t(`overview.${info.kind === 'regular' ? 'regular' : 'index'}`)}</Badge>
          {info.file && <Chip label={t('overview.file')} value={info.file} mono />}
          {info.sourceRoot && <Chip label={t('overview.sourceRoot')} value={info.sourceRoot} mono />}
          <Chip label={t('overview.sources')} value={String(info.sources.length || (info.sections?.length ?? 0))} />
          <Chip label={t('overview.names')} value={String(info.names)} />
          <Chip label={t('overview.segments')} value={String(info.segments)} />
          <Chip label={t('overview.mappedLines')} value={String(info.mappedLines)} />
          {contentCovered !== null && (
            <Chip
              label={t('overview.content')}
              value={t('overview.contentCovered', { covered: contentCovered, total: info.sources.length })}
            />
          )}
          <span className="ml-auto flex items-center gap-2">
            <Button size="sm" variant="soft" onClick={() => void openFile()}>
              {t('open.openFile')}
            </Button>
          </span>
        </div>
        {info.diagnostics.length > 0 && (
          <ul className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-warning">
            {info.diagnostics.map((diag, index) => (
              <li key={index} className="flex items-center gap-1.5">
                <TriangleAlert className="size-3.5" />
                {errorMessage(diag)}
              </li>
            ))}
          </ul>
        )}
        {loading.running && (
          <div className="flex items-center gap-3">
            <Progress
              value={loading.progress ? (loading.progress.done / Math.max(loading.progress.total, 1)) * 100 : null}
              aria-label={t('open.loading')}
            />
            <span className="shrink-0 text-xs text-fg-muted">{t('open.loading')}</span>
            <Button size="sm" onClick={() => void loading.cancel()}>
              {t('open.cancel')}
            </Button>
          </div>
        )}
      </section>

      <SegmentedControl<'stack' | 'position'>
        value={mode}
        onValueChange={setMode}
        options={[
          { value: 'stack', label: t('tabs.stack') },
          { value: 'position', label: t('tabs.position') },
        ]}
        className="shrink-0"
      />

      {mode === 'stack' && (
        <div className="grid min-h-0 flex-1 grid-cols-[minmax(280px,0.9fr)_minmax(0,1.1fr)] gap-3">
          <Panel
            icon={<FileCode2 />}
            title={t('stack.title')}
            footer={
              <div className="flex w-full items-center gap-2">
                <span className="shrink-0 text-xs text-fg-subtle">{t('stack.asset')}</span>
                <Input
                  value={asset}
                  onChange={(event) => setAssetDraft({ handle: info.handle, value: event.target.value })}
                  placeholder={t('stack.assetPlaceholder')}
                  wrapperClassName="min-w-0 flex-1"
                  size="sm"
                  aria-label={t('stack.asset')}
                  spellCheck={false}
                />
                <Button size="sm" onClick={() => void symbolicate()} disabled={busy}>
                  {busy && <Spinner className="size-3.5" />}
                  {t('stack.run')}
                </Button>
              </div>
            }
          >
            <div className="flex h-full min-h-0 flex-col gap-2">
              <div className="min-h-0 flex-1 overflow-hidden rounded-control border border-border">
                <CodeEditor
                  value={stack}
                  onChange={setStack}
                  language="text"
                  placeholder={t('stack.placeholder')}
                  aria-label={t('stack.title')}
                />
              </div>
              <p className="text-xs text-fg-subtle">{t('stack.assetHint')}</p>
            </div>
          </Panel>

          <div className="grid min-h-0 grid-rows-[minmax(0,1fr)_auto] gap-3">
            <Panel
              icon={<ListTree />}
              title={t('frames.title')}
              extra={
                result && (
                  <span className="flex items-center gap-1.5">
                    <Badge variant="success">
                      {`${t('frames.status.resolved')} ${result.stats.resolved}`}
                    </Badge>
                    {result.stats.no_mapping > 0 && (
                      <Badge variant="danger">
                        {`${t('frames.status.no_mapping')} ${result.stats.no_mapping}`}
                      </Badge>
                    )}
                    {result.stats.unrelated + result.stats.unassociated > 0 && (
                      <Badge variant="warning">
                        {`${t('frames.status.unrelated')} ${result.stats.unrelated + result.stats.unassociated}`}
                      </Badge>
                    )}
                  </span>
                )
              }
            >
              <div className="h-full min-h-0 overflow-y-auto">
                {!result && !busy && (
                  <div className="flex h-full items-center justify-center">
                    <Empty icon={<Frame />} title={t('frames.empty')} />
                  </div>
                )}
                {busy && (
                  <div className="flex items-center justify-center gap-2 py-6 text-xs text-fg-muted">
                    <Spinner className="size-3.5" />
                    {t('stack.run')}
                  </div>
                )}
                {failure && (
                  <p className="flex items-center gap-1.5 px-3 py-3 text-xs text-danger">
                    <TriangleAlert className="size-3.5" />
                    {errorMessage(failure)}
                  </p>
                )}
                {result?.frames.map((frame) => {
                  const resolved = frame.resolved
                  return (
                    <button
                      key={frame.index}
                      type="button"
                      onClick={() => setSelected(frame.index)}
                      className={cn(
                        'flex w-full items-start gap-3 rounded-control px-3 py-2 text-left',
                        frame.index === Math.min(selected, result.frames.length - 1) ? 'bg-hover' : 'hover:bg-hover',
                      )}
                    >
                      <span className="w-6 shrink-0 pt-0.5 text-right font-mono text-xs text-fg-subtle">
                        {frame.index + 1}
                      </span>
                      <span className="min-w-0 flex-1 space-y-0.5">
                        <span className="block truncate font-mono text-xs text-fg-muted" title={frame.raw}>
                          {frame.raw}
                        </span>
                        {resolved && (
                          <span className="block truncate font-mono text-xs text-fg" data-selectable>
                            {t('frames.resolvedFrame', {
                              source: resolved.source ?? t('frames.anonymous'),
                              line: resolved.line,
                              column: resolved.column,
                            })}
                          </span>
                        )}
                        {frame.reason && (
                          <span className="block truncate text-xs text-fg-subtle">{errorMessage(frame.reason)}</span>
                        )}
                      </span>
                      <span className="shrink-0 pt-0.5">
                        <StatusBadge status={frame.status} />
                      </span>
                    </button>
                  )
                })}
                {result?.diagnostics.map((diag, index) => (
                  <p key={index} className="px-3 py-1 text-xs text-warning">
                    {errorMessage(diag)}
                  </p>
                ))}
              </div>
            </Panel>

            <section className="rounded-card border border-border bg-surface p-3 shadow-card">
              {selectedFrame ? (
                <FrameDetail frame={selectedFrame} />
              ) : (
                <p className="text-xs text-fg-subtle">{t('frames.empty')}</p>
              )}
            </section>
          </div>
        </div>
      )}

      {mode === 'position' && (
        <section className="shrink-0 space-y-3 rounded-card border border-border bg-surface p-4 shadow-card">
          <div className="flex flex-wrap items-end gap-4">
            <label className="flex flex-col gap-1.5 text-xs text-fg-muted">
              {t('position.line')}
              <NumberInput
                value={position.line}
                onValueChange={(line) => setPosition((prev) => ({ ...prev, line }))}
                min={1}
                aria-label={t('position.line')}
              />
            </label>
            <label className="flex flex-col gap-1.5 text-xs text-fg-muted">
              {t('position.column')}
              <NumberInput
                value={position.column}
                onValueChange={(column) => setPosition((prev) => ({ ...prev, column }))}
                min={1}
                aria-label={t('position.column')}
              />
            </label>
            <Button onClick={() => void symbolicate()} disabled={busy}>
              {busy && <Spinner className="size-3.5" />}
              {t('position.run')}
            </Button>
            <p className="text-xs text-fg-subtle">{t('position.hint')}</p>
          </div>
          {failure && (
            <p className="flex items-center gap-1.5 text-xs text-danger">
              <TriangleAlert className="size-3.5" />
              {errorMessage(failure)}
            </p>
          )}
          {selectedFrame ? (
            <FrameDetail frame={selectedFrame} />
          ) : (
            result && <p className="text-xs text-fg-subtle">{t('frames.empty')}</p>
          )}
        </section>
      )}

      <section className="shrink-0 space-y-2 rounded-card border border-border bg-surface p-3 shadow-card">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-xs text-fg-subtle">{t('tabs.sources')}</span>
          <span className="ml-auto flex items-center gap-2">
            <Button size="sm" variant="soft" disabled={!result} onClick={() => void copyReport('markdown')}>
              {t('export.markdown')}
            </Button>
            <Button size="sm" variant="soft" disabled={!result} onClick={() => void copyReport('json')}>
              {t('export.json')}
            </Button>
            <Button size="sm" disabled={!result} onClick={() => void saveReport('markdown')}>
              {t('export.saveMd')}
            </Button>
            <Button size="sm" disabled={!result} onClick={() => void saveReport('json')}>
              {t('export.saveJson')}
            </Button>
          </span>
        </div>
        {info.sections ? (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-xs">
              <thead>
                <tr className="text-fg-subtle">
                  <th className="py-1 pr-4 font-medium">{t('overview.section.index')}</th>
                  <th className="py-1 pr-4 font-medium">{t('overview.section.offset')}</th>
                  <th className="py-1 pr-4 font-medium">{t('overview.section.sources')}</th>
                  <th className="py-1 font-medium">{t('overview.section.segments')}</th>
                </tr>
              </thead>
              <tbody>
                {info.sections.map((section) => (
                  <tr key={section.index} className="border-t border-border text-fg">
                    <td className="py-1 pr-4 font-mono">{section.index + 1}</td>
                    <td className="py-1 pr-4 font-mono">
                      {section.offsetLine}:{section.offsetColumn}
                    </td>
                    <td className="py-1 pr-4">{section.sources}</td>
                    <td className="py-1">{section.segments}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : info.sources.length > 0 ? (
          <div className="max-h-40 overflow-y-auto">
            {info.sources.map((source, index) => (
              <div key={index} className="flex items-center gap-3 py-1 text-xs">
                <span className="min-w-0 flex-1 truncate font-mono text-fg" data-selectable>
                  {source.name ?? t('frames.anonymous')}
                </span>
                <span className="shrink-0 text-fg-subtle">
                  {t('sources.content')} {source.hasContent ? t('sources.yes') : t('sources.no')}
                </span>
                <span className="shrink-0 text-fg-subtle">
                  {t('sources.segments')} {source.segments}
                </span>
              </div>
            ))}
          </div>
        ) : (
          <p className="text-xs text-fg-subtle">{t('sources.empty')}</p>
        )}
      </section>

      <Modal open={pasteOpen} onOpenChange={setPasteOpen} size="lg" title={t('open.pasteTitle')}>
        <div className="space-y-3">
          <div className="h-64 overflow-hidden rounded-control border border-border">
            <CodeEditor
              value={pasteText}
              onChange={setPasteText}
              language="json"
              placeholder='{"version":3, …}'
              aria-label={t('open.pasteTitle')}
            />
          </div>
          <div className="flex justify-end gap-2">
            <Button onClick={() => void parsePaste()} disabled={pasteText.trim().length === 0}>
              {t('open.parse')}
            </Button>
          </div>
        </div>
      </Modal>
    </div>
  )
}
