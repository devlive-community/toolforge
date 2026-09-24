import { useEffect, useState, type ReactNode } from 'react'
import { Badge, Button, Input, LogViewer, Panel, Progress, SegmentedControl, Tooltip, cn, toast } from '@toolforge/ui'
import { ResourceItem, formatBytes, host, usePlugin, useResources, useTask } from '@toolforge/plugin-ui-sdk'
import { Brain, FolderOpen, FolderSearch, ImagePlus, Images, Play, ScrollText, Settings2, Square, TriangleAlert, Upload, X } from 'lucide-react'
import { MODELS, type BackgroundMode, type FileResult, type Format, type ModelId, type Report } from './types'

const IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp', 'bmp', 'gif', 'tif', 'tiff']
const WHITE = '#ffffff' // tf-allow: 白色背景选项的取值
const baseName = (path: string) => path.split(/[\\/]/).pop() ?? path
const isImage = (path: string) => IMAGE_EXTENSIONS.includes(path.split('.').pop()?.toLowerCase() ?? '')

/** 透明区域显示为棋盘格 */
const CHECKERBOARD =
  'bg-[conic-gradient(var(--color-border)_25%,var(--color-surface)_0_50%,var(--color-border)_0_75%,var(--color-surface)_0)] bg-[length:14px_14px]'

function Field({ label, children }: { label: ReactNode; children: ReactNode }) {
  return (
    <div className="space-y-1.5">
      <p className="text-xs font-medium text-fg-muted">{label}</p>
      {children}
    </div>
  )
}

export function BackgroundRemover() {
  const { t, errorMessage } = usePlugin()
  const resources = useResources()
  const task = useTask<Report>()
  const [picked, setPicked] = useState<ModelId | null>(null)
  const [paths, setPaths] = useState<string[]>([])
  const [hovering, setHovering] = useState(false)
  const [mode, setMode] = useState<BackgroundMode>('transparent')
  const [color, setColor] = useState('#16a34a') // tf-allow: 自定义背景色的默认输入值
  const [format, setFormat] = useState<Format>('png')
  const [outputDir, setOutputDir] = useState<string | null>(null)

  // 未手动选择时，优先使用已下载的模型
  const model: ModelId = picked ?? (MODELS.find((id) => resources.get(id)?.installed) as ModelId | undefined) ?? 'u2netp'
  const ready = resources.get(model)?.installed ?? false

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

  const background = mode === 'transparent' ? null : mode === 'white' ? WHITE : color
  const start = () => task.start('remove', { paths, model, background, format: background ? format : 'png', outputDir })

  const progress = task.progress && task.progress.total > 0 ? (task.progress.done / task.progress.total) * 100 : null
  const report = task.result

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(300px,360px)_minmax(0,1fr)] gap-3">
      <div className="flex min-h-0 flex-col gap-3 overflow-auto">
        <Panel icon={<Brain />} title={t('model.title')} className="shrink-0" bodyClassName="space-y-2 p-3">
          {MODELS.map((id) => (
            <ResourceItem key={id} resources={resources} id={id} selected={model === id} onSelect={() => setPicked(id)} />
          ))}
          <p className="text-[11px] text-fg-subtle">{t('model.hint')}</p>
        </Panel>

        <Panel icon={<Settings2 />} title={t('options.title')} className="shrink-0" bodyClassName="space-y-3.5 p-3">
          <Field label={t('options.background')}>
            <SegmentedControl<BackgroundMode>
              size="sm"
              value={mode}
              onValueChange={setMode}
              aria-label={t('options.background')}
              options={[
                { value: 'transparent', label: t('options.transparent') },
                { value: 'white', label: t('options.white') },
                { value: 'custom', label: t('options.custom') },
              ]}
            />
          </Field>
          {mode === 'custom' && (
            <Input
              size="sm"
              value={color}
              onChange={(event) => setColor(event.target.value)}
              className="font-mono"
              aria-label={t('options.custom')}
              leading={<span className="size-3.5 rounded-sm border border-border" style={{ backgroundColor: color }} />}
            />
          )}
          {mode !== 'transparent' && (
            <Field label={t('options.format')}>
              <SegmentedControl<Format>
                size="sm"
                value={format}
                onValueChange={setFormat}
                aria-label={t('options.format')}
                options={[
                  { value: 'png', label: 'PNG' },
                  { value: 'jpeg', label: 'JPEG' },
                ]}
              />
            </Field>
          )}
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

          {task.running && (
            <div className="space-y-1.5">
              <Progress value={progress} />
              <p className="text-xs text-fg-muted tabular-nums">
                {task.stage === 'bg.load_model' ? t('stages.bg.load_model') : t('files.progress', { done: task.progress?.done ?? 0, total: task.progress?.total ?? paths.length })}
              </p>
            </div>
          )}
          {task.running ? (
            <Button block onClick={task.cancel}>
              <Square />
              {t('files.cancel')}
            </Button>
          ) : (
            <Button block variant="primary" disabled={paths.length === 0 || !ready} onClick={start}>
              <Play />
              {t('files.start')}
            </Button>
          )}
          {!ready && <p className="text-[11px] text-warning">{t('model.required')}</p>}
        </Panel>
      </div>

      <div className="grid min-h-0 grid-rows-[minmax(0,1fr)_minmax(150px,0.45fr)] gap-3">
        <Panel
          icon={<Images />}
          title={report ? t('results.title') : t('files.title')}
          extra={
            report ? (
              <Badge variant="success">{t('results.summary', { count: report.succeeded, total: report.files.length, ms: report.elapsedMs })}</Badge>
            ) : (
              paths.length > 0 && <Badge>{paths.length}</Badge>
            )
          }
          actions={
            <>
              {report && (
                <Button size="sm" variant="ghost" onClick={task.reset} disabled={task.running}>
                  {t('results.back')}
                </Button>
              )}
              <Button size="sm" onClick={pick} disabled={task.running}>
                <ImagePlus />
                {t('files.add')}
              </Button>
              <Button
                size="sm"
                onClick={() => {
                  setPaths([])
                  task.reset()
                }}
                disabled={task.running || paths.length === 0}
              >
                {t('common:clear')}
              </Button>
            </>
          }
          bodyClassName="overflow-auto p-2"
        >
          {task.status === 'failed' && task.error ? (
            <p className="p-4 text-[13px] text-danger">{errorMessage(task.error)}</p>
          ) : report ? (
            <div className="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-2">
              {report.files.map((file) => (
                <ResultCard key={file.path} file={file} />
              ))}
            </div>
          ) : paths.length === 0 ? (
            <button
              type="button"
              onClick={pick}
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
        <Panel icon={<ScrollText />} title={t('logs.title')} extra={task.logs.length > 0 && <Badge>{task.logs.length}</Badge>}>
          <LogViewer entries={task.logs} emptyText={t('logs.empty')} jumpLabel={t('logs.jumpLatest')} />
        </Panel>
      </div>
    </div>
  )
}

function ResultCard({ file }: { file: FileResult }) {
  const { t, errorMessage } = usePlugin()
  const reveal = async (path: string) => {
    try {
      await host.revealPath(path)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  return (
    <section className="overflow-hidden rounded-card border border-border">
      <div className={cn('flex aspect-[4/3] items-center justify-center', file.transparent ? CHECKERBOARD : 'bg-surface-2')}>
        {file.preview ? (
          <img src={file.preview} alt={file.name} className="max-h-full max-w-full object-contain" />
        ) : (
          <TriangleAlert className="size-6 text-danger" />
        )}
      </div>
      <div className="flex items-start gap-1.5 border-t border-border p-2.5">
        <div className="min-w-0 flex-1 space-y-0.5">
          <p className="truncate text-[13px] font-medium text-fg" title={file.output ?? file.name}>
            {file.output ? baseName(file.output) : file.name}
          </p>
          {file.error ? (
            <p className="text-xs text-danger">{errorMessage(file.error)}</p>
          ) : (
            <p className="text-xs text-fg-muted tabular-nums">
              {t('results.meta', { width: file.width, height: file.height, size: formatBytes(file.bytes), ms: file.ms })}
            </p>
          )}
        </div>
        {file.output && (
          <Tooltip content={t('results.reveal')}>
            <Button size="icon-sm" variant="ghost" aria-label={t('results.reveal')} onClick={() => reveal(file.output!)}>
              <FolderSearch />
            </Button>
          </Tooltip>
        )}
      </div>
    </section>
  )
}
