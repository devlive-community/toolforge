import { useEffect, useState, type ReactNode } from 'react'
import { Badge, Button, Empty, Input, LogViewer, NumberInput, Panel, Progress, SegmentedControl, Select, Tooltip, cn, toast } from '@toolforge/ui'
import { host, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { FolderOpen, FolderSearch, ImagePlus, Images, Play, ScrollText, Settings2, Square, TriangleAlert, Upload, X } from 'lucide-react'
import { formatBytes } from './format'
import { TARGETS, TARGET_LABELS, type FileResult, type Report, type Resize, type ResizeMode, type Target } from './types'

const IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp', 'ico', 'tif', 'tiff']
const baseName = (path: string) => path.split(/[\\/]/).pop() ?? path
const isImage = (path: string) => IMAGE_EXTENSIONS.includes(path.split('.').pop()?.toLowerCase() ?? '')

function Field({ label, children }: { label: ReactNode; children: ReactNode }) {
  return (
    <div className="space-y-1.5">
      <p className="text-xs font-medium text-fg-muted">{label}</p>
      {children}
    </div>
  )
}

export function ImageConverter() {
  const { t, errorMessage } = usePlugin()
  const task = useTask<Report>()
  const [paths, setPaths] = useState<string[]>([])
  const [hovering, setHovering] = useState(false)
  const [target, setTarget] = useState<Target>('jpeg')
  const [quality, setQuality] = useState(80)
  const [resizeMode, setResizeMode] = useState<ResizeMode>('none')
  const [percent, setPercent] = useState(50)
  const [maxWidth, setMaxWidth] = useState(1920)
  const [maxHeight, setMaxHeight] = useState(0)
  const [outputDir, setOutputDir] = useState<string | null>(null)
  const [suffix, setSuffix] = useState('')

  const addPaths = (next: string[]) => setPaths((prev) => [...prev, ...next.filter((p) => isImage(p) && !prev.includes(p))])

  useEffect(() => {
    const off = host.onFileDrop({ drop: addPaths, over: setHovering })
    return () => {
      off.then((unlisten) => unlisten())
    }
  }, [])

  const run = async (action: () => Promise<void>) => {
    try {
      await action()
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const pick = () => run(async () => addPaths(await host.dialog.openFiles([{ name: t('files.filter'), extensions: IMAGE_EXTENSIONS }])))
  const pickDir = () =>
    run(async () => {
      const dir = await host.dialog.openDirectory()
      if (dir) setOutputDir(dir)
    })

  const resize: Resize =
    resizeMode === 'scale' ? { mode: 'scale', percent } : resizeMode === 'fit' ? { mode: 'fit', maxWidth, maxHeight } : { mode: 'none' }

  const start = () => task.start('convert', { paths, target, quality, resize, suffix, outputDir })

  const progress = task.progress && task.progress.total > 0 ? (task.progress.done / task.progress.total) * 100 : null
  const report = task.result
  const saved = report && report.totalBefore > 0 ? 1 - report.totalAfter / report.totalBefore : null

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(300px,0.8fr)_minmax(0,1.6fr)] gap-3">
      <div className="flex min-h-0 flex-col gap-3">
        <Panel
          icon={<Images />}
          title={t('files.title')}
          extra={paths.length > 0 && <Badge>{paths.length}</Badge>}
          actions={
            <>
              <Button size="sm" onClick={pick} disabled={task.running}>
                <ImagePlus />
                {t('files.add')}
              </Button>
              <Button size="sm" onClick={() => setPaths([])} disabled={task.running || paths.length === 0}>
                {t('common:clear')}
              </Button>
            </>
          }
          className="flex-1"
          bodyClassName="overflow-auto p-2"
        >
          {paths.length === 0 ? (
            <button
              type="button"
              onClick={pick}
              className={cn(
                'flex h-full min-h-32 w-full flex-col items-center justify-center gap-2 rounded-card border-2 border-dashed text-center outline-none transition-colors',
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
              {paths.map((path) => (
                <li key={path} className="group flex items-center gap-2 rounded-control px-2.5 py-1.5 hover:bg-hover">
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-[13px] text-fg">{baseName(path)}</p>
                    <p className="truncate text-[11px] text-fg-subtle" data-selectable>
                      {path}
                    </p>
                  </div>
                  <Tooltip content={t('files.remove')}>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label={t('files.remove')}
                      disabled={task.running}
                      className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                      onClick={() => setPaths((prev) => prev.filter((p) => p !== path))}
                    >
                      <X />
                    </Button>
                  </Tooltip>
                </li>
              ))}
            </ul>
          )}
        </Panel>

        <Panel icon={<Settings2 />} title={t('options.title')} className="shrink-0" bodyClassName="space-y-3.5 p-3">
          <div className="grid grid-cols-2 gap-3">
            <Field label={t('options.format')}>
              <Select<Target>
                value={target}
                onValueChange={setTarget}
                aria-label={t('options.format')}
                options={TARGETS.map((value) => ({ value, label: value === 'keep' ? t('options.keep') : TARGET_LABELS[value] }))}
              />
            </Field>
            <Field label={t('options.quality')}>
              <NumberInput
                value={quality}
                onValueChange={setQuality}
                min={1}
                max={100}
                disabled={target !== 'jpeg' && target !== 'keep'}
                aria-label={t('options.quality')}
              />
            </Field>
          </div>
          {(target === 'webp' || target === 'png') && <p className="-mt-1.5 text-[11px] text-fg-subtle">{t(`options.hint.${target}`)}</p>}

          <Field label={t('options.resize')}>
            <SegmentedControl<ResizeMode>
              size="sm"
              value={resizeMode}
              onValueChange={setResizeMode}
              aria-label={t('options.resize')}
              options={[
                { value: 'none', label: t('resize.none') },
                { value: 'scale', label: t('resize.scale') },
                { value: 'fit', label: t('resize.fit') },
              ]}
            />
          </Field>
          {resizeMode === 'scale' && (
            <Field label={t('resize.percent')}>
              <NumberInput value={percent} onValueChange={setPercent} min={1} max={1000} step={5} aria-label={t('resize.percent')} />
            </Field>
          )}
          {resizeMode === 'fit' && (
            <div className="grid grid-cols-2 gap-3">
              <Field label={t('resize.maxWidth')}>
                <NumberInput value={maxWidth} onValueChange={setMaxWidth} min={0} max={20000} step={10} aria-label={t('resize.maxWidth')} />
              </Field>
              <Field label={t('resize.maxHeight')}>
                <NumberInput value={maxHeight} onValueChange={setMaxHeight} min={0} max={20000} step={10} aria-label={t('resize.maxHeight')} />
              </Field>
            </div>
          )}

          <div className="grid grid-cols-[minmax(0,1fr)_120px] gap-3">
            <Field label={t('options.output')}>
              <div className="flex items-center gap-1.5">
                <Button className="min-w-0 flex-1 justify-start" onClick={pickDir}>
                  <FolderOpen />
                  <span className="truncate">{outputDir ?? t('options.sameDir')}</span>
                </Button>
                {outputDir && (
                  <Tooltip content={t('options.resetDir')}>
                    <Button size="icon-md" variant="ghost" aria-label={t('options.resetDir')} onClick={() => setOutputDir(null)}>
                      <X />
                    </Button>
                  </Tooltip>
                )}
              </div>
            </Field>
            <Field label={t('options.suffix')}>
              <Input value={suffix} onChange={(event) => setSuffix(event.target.value)} placeholder="-min" aria-label={t('options.suffix')} />
            </Field>
          </div>

          {task.running && (
            <div className="space-y-1.5">
              <Progress value={progress} />
              <p className="text-xs text-fg-muted tabular-nums">
                {t('files.progress', { done: task.progress?.done ?? 0, total: task.progress?.total ?? paths.length })}
              </p>
            </div>
          )}
          {task.running ? (
            <Button block onClick={task.cancel}>
              <Square />
              {t('files.cancel')}
            </Button>
          ) : (
            <Button block variant="primary" disabled={paths.length === 0} onClick={start}>
              <Play />
              {t('files.start')}
            </Button>
          )}
        </Panel>
      </div>

      <div className="grid min-h-0 grid-rows-[minmax(0,1fr)_minmax(160px,0.55fr)] gap-3">
        <Panel
          icon={<Images />}
          title={t('results.title')}
          extra={
            task.status === 'succeeded' && report ? (
              <Badge variant="success">
                {t('results.summary', { count: report.succeeded, total: report.files.length, ms: report.elapsedMs })}
              </Badge>
            ) : task.status === 'cancelled' ? (
              <Badge>{t('results.cancelled')}</Badge>
            ) : null
          }
          footer={
            report &&
            report.succeeded > 0 && (
              <>
                <span>{`${formatBytes(report.totalBefore)} → ${formatBytes(report.totalAfter)}`}</span>
                {saved !== null && (
                  <span className={cn('ml-auto font-medium', saved >= 0 ? 'text-success' : 'text-warning')}>
                    {t(saved >= 0 ? 'results.saved' : 'results.grew', { percent: Math.abs(Math.round(saved * 100)) })}
                  </span>
                )}
              </>
            )
          }
          bodyClassName="overflow-auto p-2"
        >
          {task.status === 'failed' && task.error ? (
            <p className="p-4 text-[13px] text-danger">{errorMessage(task.error)}</p>
          ) : !report ? (
            <Empty icon={<Images />} title={task.running ? t('results.running') : t('results.empty')} description={task.running ? undefined : t('results.emptyHint')} />
          ) : (
            <div className="grid grid-cols-[repeat(auto-fill,minmax(260px,1fr))] gap-2">
              {report.files.map((file) => (
                <ResultCard key={file.path} file={file} />
              ))}
            </div>
          )}
        </Panel>
        <Panel icon={<ScrollText />} title={t('logs.title')} extra={task.logs.length > 0 && <Badge>{task.logs.length}</Badge>}>
          <LogViewer entries={task.logs} emptyText={t('logs.empty')} jumpLabel={t('logs.jumpLatest')} />
        </Panel>
      </div>
    </div>
  )
}

function ResultCard({ file }: { file: FileResult }) {
  const { t, errorMessage } = usePlugin()
  const change = file.beforeBytes > 0 ? file.afterBytes / file.beforeBytes - 1 : 0
  const reveal = async (path: string) => {
    try {
      await host.revealPath(path)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  return (
    <section className="flex gap-3 rounded-card border border-border p-2.5">
      <div className="flex size-16 shrink-0 items-center justify-center overflow-hidden rounded-control border border-border bg-surface-2">
        {file.thumbnail ? (
          <img src={file.thumbnail} alt="" className="max-h-full max-w-full object-contain" />
        ) : (
          <TriangleAlert className={cn('size-5', file.error ? 'text-danger' : 'text-fg-subtle')} />
        )}
      </div>
      <div className="min-w-0 flex-1 space-y-1">
        <div className="flex items-center gap-1.5">
          <p className="min-w-0 flex-1 truncate text-[13px] font-medium text-fg" title={file.name}>
            {file.output ? baseName(file.output) : file.name}
          </p>
          {file.output && (
            <Tooltip content={t('results.reveal')}>
              <Button size="icon-sm" variant="ghost" aria-label={t('results.reveal')} onClick={() => reveal(file.output!)}>
                <FolderSearch />
              </Button>
            </Tooltip>
          )}
        </div>
        {file.error ? (
          <p className="text-xs text-danger">{errorMessage(file.error)}</p>
        ) : (
          <>
            <p className="text-xs text-fg-muted tabular-nums">
              {`${file.width}×${file.height}`}
              {(file.width !== file.newWidth || file.height !== file.newHeight) && ` → ${file.newWidth}×${file.newHeight}`}
              {file.format && ` · ${TARGET_LABELS[file.format as Exclude<Target, 'keep'>] ?? file.format}`}
            </p>
            <p className="flex items-center gap-2 text-xs text-fg-muted tabular-nums">
              <span>{`${formatBytes(file.beforeBytes)} → ${formatBytes(file.afterBytes)}`}</span>
              <Badge variant={change <= 0 ? 'success' : 'warning'}>
                {change <= 0 ? '−' : '+'}
                {Math.abs(Math.round(change * 100))}%
              </Badge>
            </p>
          </>
        )}
      </div>
    </section>
  )
}
