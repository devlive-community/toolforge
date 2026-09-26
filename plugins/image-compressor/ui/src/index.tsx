import { useEffect, useMemo, useState, type ReactNode } from 'react'
import { Badge, Button, Empty, Input, LogViewer, Modal, NumberInput, Panel, Progress, SegmentedControl, Select, Switch, Tooltip, cn, toast } from '@toolforge/ui'
import { formatBytes, host, usePlugin, usePluginState, useTask } from '@toolforge/plugin-ui-sdk'
import { Columns2, FolderOpen, FolderPlus, FolderSearch, ImageDown, ImagePlus, Play, ScrollText, Settings2, Square, Upload, X } from 'lucide-react'
import { CompareView, type CompareImages } from './CompareView'
import {
  COLORS,
  DEFAULT_OPTIONS,
  DEFAULT_OUTPUT,
  FORMAT_LABELS,
  MAX_SIZES,
  type Entry,
  type FileResult,
  type Format,
  type Options,
  type Output,
  type PngMode,
  type Report,
} from './types'

const EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp']
const extension = (name: string) => name.split('.').pop()?.toLowerCase() ?? ''

function Field({ label, hint, children }: { label: ReactNode; hint?: ReactNode; children: ReactNode }) {
  return (
    <div className="space-y-1.5">
      <p className="text-xs font-medium text-fg-muted">{label}</p>
      {children}
      {hint && <p className="text-[11px] leading-snug text-fg-subtle">{hint}</p>}
    </div>
  )
}

function Saving({ before, after }: { before: number; after: number }) {
  const change = before > 0 ? after / before - 1 : 0
  return (
    <Badge variant={change < 0 ? 'success' : 'neutral'}>
      {change < 0 ? '−' : ''}
      {Math.abs(Math.round(change * 100))}%
    </Badge>
  )
}

function ImageCompressor() {
  const { t, call, errorMessage } = usePlugin()
  const task = useTask<Report>()
  const [options, setOptions] = usePluginState<Options>('options', DEFAULT_OPTIONS)
  const [output, setOutput] = usePluginState<Output>('output', DEFAULT_OUTPUT)
  const [files, setFiles] = useState<Entry[]>([])
  const [hovering, setHovering] = useState(false)
  const [comparing, setComparing] = useState<{ file: FileResult; images: CompareImages | null } | null>(null)
  const update = (patch: Partial<Options>) => setOptions({ ...options, ...patch })

  const report = task.result
  const results = useMemo(() => new Map((report?.files ?? []).map((file) => [file.path, file])), [report])

  const addPaths = async (paths: string[]) => {
    if (paths.length === 0) return
    try {
      const found = await call<Entry[]>('scan', { paths })
      setFiles((prev) => {
        const known = new Set(prev.map((f) => f.path))
        return [...prev, ...found.filter((f) => !known.has(f.path))]
      })
      if (found.length === 0) toast.error(t('files.noImages'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  useEffect(() => {
    const off = host.onFileDrop({ drop: (paths) => void addPaths(paths), over: setHovering })
    return () => {
      off.then((unlisten) => unlisten())
    }
    // addPaths 只依赖稳定的 call / t
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  useEffect(() => {
    if (task.status === 'failed' && task.error) toast.error(errorMessage(task.error))
  }, [task.status, task.error, errorMessage])

  const guard = async (action: () => Promise<void>) => {
    try {
      await action()
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const pickFiles = () => guard(async () => addPaths(await host.dialog.openFiles([{ name: t('files.filter'), extensions: EXTENSIONS }])))
  const pickFolder = () =>
    guard(async () => {
      const dir = await host.dialog.openDirectory()
      if (dir) await addPaths([dir])
    })
  const pickOutput = () =>
    guard(async () => {
      const dir = await host.dialog.openDirectory()
      if (dir) setOutput({ ...output, dir })
    })

  const start = () => task.start('compress', { paths: files.map((f) => f.path), options, outputDir: output.dir, suffix: output.suffix })

  const openCompare = (file: FileResult) => {
    if (!file.output) return
    setComparing({ file, images: null })
    call<CompareImages>('compare', { original: file.path, output: file.output })
      .then((images) => setComparing((prev) => (prev?.file === file ? { file, images } : prev)))
      .catch((error) => {
        setComparing(null)
        toast.error(errorMessage(error))
      })
  }

  const progress = task.progress && task.progress.total > 0 ? (task.progress.done / task.progress.total) * 100 : null
  const totalSize = files.reduce((sum, f) => sum + f.size, 0)
  const pngOutput = options.format === 'png' || (options.format === 'keep' && files.some((f) => extension(f.name) === 'png'))
  const lossyQuality = options.format !== 'png'

  return (
    <div className="grid h-full min-h-0 grid-cols-[300px_minmax(0,1fr)] gap-3">
      <Panel icon={<Settings2 />} title={t('options.title')} bodyClassName="space-y-4 overflow-auto p-3">
        <Field label={t('options.format')}>
          <Select<Format>
            value={options.format}
            onValueChange={(format) => update({ format })}
            aria-label={t('options.format')}
            options={(['keep', 'jpeg', 'png', 'webp'] as Format[]).map((value) => ({ value, label: value === 'keep' ? t('options.keep') : FORMAT_LABELS[value] }))}
          />
        </Field>
        {lossyQuality && (
          <Field label={t('options.quality')} hint={t('options.qualityHint')}>
            <NumberInput value={options.quality} onValueChange={(quality) => update({ quality })} min={1} max={100} step={5} aria-label={t('options.quality')} />
          </Field>
        )}
        {pngOutput && (
          <>
            <Field label={t('options.png')} hint={t(`options.pngHint.${options.pngMode}`)}>
              <SegmentedControl<PngMode>
                size="sm"
                value={options.pngMode}
                onValueChange={(pngMode) => update({ pngMode })}
                aria-label={t('options.png')}
                options={[
                  { value: 'lossy', label: t('options.lossy') },
                  { value: 'lossless', label: t('options.lossless') },
                ]}
              />
            </Field>
            {options.pngMode === 'lossy' && (
              <div className="grid grid-cols-[1fr_auto] items-end gap-3">
                <Field label={t('options.colors')}>
                  <Select<number>
                    value={options.colors}
                    onValueChange={(colors) => update({ colors })}
                    aria-label={t('options.colors')}
                    options={COLORS.map((value) => ({ value, label: String(value) }))}
                  />
                </Field>
                <label className="flex h-control-md items-center gap-2 text-[13px] text-fg">
                  <Switch size="sm" checked={options.dither} onCheckedChange={(dither) => update({ dither })} aria-label={t('options.dither')} />
                  {t('options.dither')}
                </label>
              </div>
            )}
          </>
        )}
        <Field label={t('options.maxSize')}>
          <Select<number>
            value={options.maxSize}
            onValueChange={(maxSize) => update({ maxSize })}
            aria-label={t('options.maxSize')}
            options={MAX_SIZES.map((value) => ({ value, label: value === 0 ? t('options.original') : t('options.longEdge', { size: value }) }))}
          />
        </Field>
        <Tooltip content={t('options.metadataHint')}>
          <label className="flex items-center gap-2 text-[13px] text-fg">
            <Switch size="sm" checked={options.keepMetadata} onCheckedChange={(keepMetadata) => update({ keepMetadata })} aria-label={t('options.metadata')} />
            {t('options.metadata')}
          </label>
        </Tooltip>

        <div className="space-y-3 border-t border-border pt-3">
          <Field label={t('options.output')}>
            <div className="flex items-center gap-1.5">
              <Button className="min-w-0 flex-1 justify-start" onClick={pickOutput}>
                <FolderOpen />
                <span className="truncate">{output.dir ?? t('options.sameDir')}</span>
              </Button>
              {output.dir && (
                <Tooltip content={t('options.resetDir')}>
                  <Button size="icon-md" variant="ghost" aria-label={t('options.resetDir')} onClick={() => setOutput({ ...output, dir: null })}>
                    <X />
                  </Button>
                </Tooltip>
              )}
            </div>
          </Field>
          <Field label={t('options.suffix')}>
            <Input value={output.suffix} onChange={(event) => setOutput({ ...output, suffix: event.target.value })} placeholder="-min" aria-label={t('options.suffix')} />
          </Field>
        </div>

        {task.running ? (
          <div className="space-y-2">
            <Progress value={progress} />
            <p className="text-xs text-fg-muted tabular-nums">{t('files.progress', { done: task.progress?.done ?? 0, total: task.progress?.total ?? files.length })}</p>
            <Button block onClick={task.cancel}>
              <Square />
              {t('files.cancel')}
            </Button>
          </div>
        ) : (
          <Button block variant="primary" disabled={files.length === 0} onClick={start}>
            <Play />
            {t('files.start', { count: files.length })}
          </Button>
        )}
      </Panel>

      <div className="grid min-h-0 grid-rows-[minmax(0,1fr)_minmax(120px,0.3fr)] gap-3">
        <Panel
          icon={<ImageDown />}
          title={t('files.title')}
          extra={files.length > 0 && <Badge>{t('files.count', { count: files.length, size: formatBytes(totalSize) })}</Badge>}
          actions={
            <>
              <Button size="sm" onClick={pickFiles} disabled={task.running}>
                <ImagePlus />
                {t('files.add')}
              </Button>
              <Button size="sm" onClick={pickFolder} disabled={task.running}>
                <FolderPlus />
                {t('files.addFolder')}
              </Button>
              <Button size="sm" onClick={() => {
                  setFiles([])
                  task.reset()
                }} disabled={task.running || files.length === 0}>
                {t('common:clear')}
              </Button>
            </>
          }
          footer={
            report &&
            report.succeeded > 0 && (
              <>
                <span className="tabular-nums">{t('results.summary', { count: report.succeeded, total: report.files.length, ms: report.elapsedMs })}</span>
                <span className="ml-auto flex items-center gap-2 tabular-nums">
                  {`${formatBytes(report.totalBefore)} → ${formatBytes(report.totalAfter)}`}
                  <span className="font-medium text-success">{t('results.saved', { size: formatBytes(Math.max(0, report.totalBefore - report.totalAfter)) })}</span>
                </span>
              </>
            )
          }
          bodyClassName="overflow-auto p-2"
        >
          {files.length === 0 ? (
            <button
              type="button"
              onClick={pickFiles}
              className={cn(
                'flex h-full min-h-40 w-full flex-col items-center justify-center gap-2 rounded-card border-2 border-dashed text-center outline-none transition-colors',
                'focus-visible:ring-2 focus-visible:ring-ring',
                hovering ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg-muted hover:border-border-strong hover:bg-hover',
              )}
            >
              <Upload className="size-6" />
              <span className="text-[13px] font-medium">{t('files.drop')}</span>
              <span className="text-xs text-fg-subtle">{t('files.dropHint')}</span>
            </button>
          ) : (
            <ul className={cn('space-y-0.5 rounded-card', hovering && 'ring-2 ring-primary')}>
              {files.map((file) => {
                const result = results.get(file.path)
                return (
                  <li key={file.path} className="group grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 rounded-control px-2.5 py-1.5 hover:bg-hover">
                    <div className="min-w-0">
                      <p className="truncate text-[13px] text-fg">{file.name}</p>
                      <p className="truncate text-[11px] text-fg-subtle" data-selectable>
                        {result?.error ? <span className="text-danger">{errorMessage(result.error)}</span> : (result?.output ?? file.path)}
                      </p>
                    </div>
                    <div className="flex items-center gap-2 text-xs text-fg-muted tabular-nums">
                      {result && !result.error ? (
                        <>
                          {result.keptOriginal ? (
                            <Tooltip content={t('results.keptHint')}>
                              <Badge>{t('results.kept')}</Badge>
                            </Tooltip>
                          ) : (
                            result.format && <span className="text-fg-subtle">{FORMAT_LABELS[result.format]}</span>
                          )}
                          <span>{`${formatBytes(result.beforeBytes)} → ${formatBytes(result.afterBytes)}`}</span>
                          <Saving before={result.beforeBytes} after={result.afterBytes} />
                          <Tooltip content={t('results.compare')}>
                            <Button size="icon-sm" variant="ghost" aria-label={t('results.compare')} onClick={() => openCompare(result)}>
                              <Columns2 />
                            </Button>
                          </Tooltip>
                          <Tooltip content={t('results.reveal')}>
                            <Button size="icon-sm" variant="ghost" aria-label={t('results.reveal')} onClick={() => guard(() => host.revealPath(result.output!))}>
                              <FolderSearch />
                            </Button>
                          </Tooltip>
                        </>
                      ) : (
                        <>
                          <span>{formatBytes(file.size)}</span>
                          <Tooltip content={t('files.remove')}>
                            <Button
                              variant="ghost"
                              size="icon-sm"
                              aria-label={t('files.remove')}
                              disabled={task.running}
                              className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                              onClick={() => setFiles((prev) => prev.filter((f) => f.path !== file.path))}
                            >
                              <X />
                            </Button>
                          </Tooltip>
                        </>
                      )}
                    </div>
                  </li>
                )
              })}
            </ul>
          )}
        </Panel>
        <Panel icon={<ScrollText />} title={t('logPanel.title')} extra={task.logs.length > 0 && <Badge>{task.logs.length}</Badge>}>
          {task.logs.length > 0 ? (
            <LogViewer entries={task.logs} emptyText={t('logPanel.empty')} jumpLabel={t('logPanel.jumpLatest')} />
          ) : (
            <Empty title={t('logPanel.empty')} />
          )}
        </Panel>
      </div>

      <Modal
        open={comparing !== null}
        onOpenChange={(open) => !open && setComparing(null)}
        title={comparing ? comparing.file.name : ''}
        closeLabel={t('common:close')}
        size="lg"
        className="h-[80vh] max-w-[min(1100px,92vw)]"
      >
        {comparing && <CompareView file={comparing.file} images={comparing.images} />}
      </Modal>
    </div>
  )
}

export default ImageCompressor
