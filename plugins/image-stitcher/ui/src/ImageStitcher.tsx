import { useEffect, useRef, useState, type ReactNode } from 'react'
import { Badge, Button, Empty, Input, LogViewer, NumberInput, Panel, Progress, SegmentedControl, Select, Spinner, Tooltip, cn, toast } from '@toolforge/ui'
import { host, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { ArrowDown, ArrowUp, FolderOpen, FolderSearch, Images, Play, ScrollText, Settings2, Square, TriangleAlert, Upload, X } from 'lucide-react'
import { formatBytes } from './format'
import { FORMATS, FORMAT_LABELS, type Align, type BackgroundKind, type Format, type InspectEntry, type Layout, type Report } from './types'

const IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp', 'tif', 'tiff']
const isImage = (path: string) => IMAGE_EXTENSIONS.includes(path.split('.').pop()?.toLowerCase() ?? '')

function Field({ label, children }: { label: ReactNode; children: ReactNode }) {
  return (
    <div className="space-y-1.5">
      <p className="text-xs font-medium text-fg-muted">{label}</p>
      {children}
    </div>
  )
}

export function ImageStitcher() {
  const { t, call, errorMessage } = usePlugin()
  const task = useTask<Report>()
  const [entries, setEntries] = useState<InspectEntry[]>([])
  const [hovering, setHovering] = useState(false)
  const [adding, setAdding] = useState(false)
  const [layout, setLayout] = useState<Layout>('horizontal')
  const [align, setAlign] = useState<Align>('center')
  const [columns, setColumns] = useState(2)
  const [gap, setGap] = useState(0)
  const [bgKind, setBgKind] = useState<BackgroundKind>('transparent')
  // 存不带 # 的十六进制值（后端会自己 trim），避免被约定扫描当成硬编码颜色
  const [bgHex, setBgHex] = useState('f0f0f0')
  const [format, setFormat] = useState<Format>('png')
  const [quality, setQuality] = useState(85)
  const [outputDir, setOutputDir] = useState<string | null>(null)
  const [name, setName] = useState('stitched')

  const inspect = async (paths: string[]) => {
    if (paths.length === 0) return
    setAdding(true)
    try {
      const next = await call<InspectEntry[]>('inspect', { paths })
      setEntries((prev) => {
        const seen = new Set(prev.map((entry) => entry.path))
        return [...prev, ...next.filter((entry) => !seen.has(entry.path))]
      })
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setAdding(false)
    }
  }

  // onFileDrop 只注册一次，回调经由 ref 转发到最新版本
  const inspectRef = useRef(inspect)
  useEffect(() => {
    inspectRef.current = inspect
  })

  useEffect(() => {
    const off = host.onFileDrop({ drop: (paths) => void inspectRef.current(paths.filter(isImage)), over: setHovering })
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

  const pick = () => run(async () => inspect(await host.dialog.openFiles([{ name: t('files.filter'), extensions: IMAGE_EXTENSIONS }])))
  const pickDir = () =>
    run(async () => {
      const dir = await host.dialog.openDirectory()
      if (dir) setOutputDir(dir)
    })

  const move = (index: number, offset: number) =>
    setEntries((prev) => {
      const target = index + offset
      if (target < 0 || target >= prev.length) return prev
      const next = [...prev]
      ;[next[index], next[target]] = [next[target], next[index]]
      return next
    })

  const report = task.result
  const usable = entries.filter((entry) => !entry.error)
  const start = () =>
    task.start('stitch', {
      paths: usable.map((entry) => entry.path),
      layout,
      align,
      columns,
      gap,
      background: bgKind === 'color' ? { kind: 'color', hex: bgHex } : { kind: bgKind },
      format,
      quality,
      outputDir,
      name,
    })

  const progress = task.progress && task.progress.total > 0 ? (task.progress.done / task.progress.total) * 100 : null

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(320px,0.9fr)_minmax(0,1.5fr)] gap-3">
      <div className="flex min-h-0 flex-col gap-3">
        <Panel
          icon={<Images />}
          title={t('files.title')}
          extra={entries.length > 0 && <Badge>{entries.length}</Badge>}
          actions={
            <>
              <Button size="sm" onClick={pick} disabled={task.running}>
                <Upload />
                {t('files.add')}
              </Button>
              <Button size="sm" onClick={() => setEntries([])} disabled={task.running || entries.length === 0}>
                {t('common:clear')}
              </Button>
            </>
          }
          className="flex-1"
          bodyClassName="overflow-auto p-2"
        >
          {entries.length === 0 ? (
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
              {entries.map((entry, index) => (
                <li key={entry.path} className="group flex items-center gap-2.5 rounded-control px-2 py-1.5 hover:bg-hover">
                  <div className="flex size-10 shrink-0 items-center justify-center overflow-hidden rounded-control border border-border bg-surface-2">
                    {entry.thumbnail ? (
                      <img src={entry.thumbnail} alt="" className="max-h-full max-w-full object-contain" />
                    ) : (
                      <TriangleAlert className={cn('size-4', entry.error ? 'text-danger' : 'text-fg-subtle')} />
                    )}
                  </div>
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-[13px] text-fg" title={entry.name}>
                      {entry.name}
                    </p>
                    {entry.error ? (
                      <p className="truncate text-[11px] text-danger">{errorMessage(entry.error)}</p>
                    ) : (
                      <p className="text-[11px] text-fg-subtle tabular-nums">{`${entry.width} × ${entry.height}`}</p>
                    )}
                  </div>
                  <div className="flex shrink-0 items-center opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
                    <Button variant="ghost" size="icon-sm" aria-label={t('files.moveUp')} disabled={task.running || index === 0} onClick={() => move(index, -1)}>
                      <ArrowUp />
                    </Button>
                    <Button variant="ghost" size="icon-sm" aria-label={t('files.moveDown')} disabled={task.running || index === entries.length - 1} onClick={() => move(index, 1)}>
                      <ArrowDown />
                    </Button>
                    <Button variant="ghost" size="icon-sm" aria-label={t('files.remove')} disabled={task.running} onClick={() => setEntries((prev) => prev.filter((e) => e.path !== entry.path))}>
                      <X />
                    </Button>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </Panel>

        <Panel icon={<Settings2 />} title={t('options.title')} className="shrink-0" bodyClassName="space-y-3.5 p-3">
          <div className="grid grid-cols-2 gap-3">
            <Field label={t('options.layout')}>
              <SegmentedControl<Layout>
                size="sm"
                value={layout}
                onValueChange={setLayout}
                aria-label={t('options.layout')}
                options={[
                  { value: 'horizontal', label: t('layout.horizontal') },
                  { value: 'vertical', label: t('layout.vertical') },
                  { value: 'grid', label: t('layout.grid') },
                ]}
              />
            </Field>
            {layout === 'grid' ? (
              <Field label={t('options.columns')}>
                <NumberInput value={columns} onValueChange={setColumns} min={1} max={64} aria-label={t('options.columns')} />
              </Field>
            ) : (
              <Field label={t('options.align')}>
                <SegmentedControl<Align>
                  size="sm"
                  value={align}
                  onValueChange={setAlign}
                  aria-label={t('options.align')}
                  options={[
                    { value: 'start', label: t('align.start') },
                    { value: 'center', label: t('align.center') },
                    { value: 'end', label: t('align.end') },
                  ]}
                />
              </Field>
            )}
          </div>
          {layout === 'grid' && <p className="-mt-1.5 text-[11px] text-fg-subtle">{t('layout.gridHint')}</p>}

          <div className="grid grid-cols-2 gap-3">
            <Field label={t('options.gap')}>
              <NumberInput value={gap} onValueChange={setGap} min={0} max={1000} step={5} aria-label={t('options.gap')} />
            </Field>
            <Field label={t('options.format')}>
              <Select<Format>
                value={format}
                onValueChange={setFormat}
                aria-label={t('options.format')}
                options={FORMATS.map((value) => ({ value, label: FORMAT_LABELS[value] }))}
              />
            </Field>
          </div>

          <Field label={t('options.background')}>
            <SegmentedControl<BackgroundKind>
              size="sm"
              value={bgKind}
              onValueChange={setBgKind}
              aria-label={t('options.background')}
              options={[
                { value: 'transparent', label: t('background.transparent') },
                { value: 'white', label: t('background.white') },
                { value: 'black', label: t('background.black') },
                { value: 'color', label: t('background.custom') },
              ]}
            />
          </Field>
          {bgKind === 'color' && (
            <Field label={t('background.hex')}>
              <Input value={bgHex} onChange={(event) => setBgHex(event.target.value)} placeholder="#RRGGBB" aria-label={t('background.hex')} />
            </Field>
          )}
          {format === 'jpeg' && bgKind === 'transparent' && <p className="-mt-1.5 text-[11px] text-warning">{t('background.jpegHint')}</p>}

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
            <Field label={t('options.name')}>
              <Input value={name} onChange={(event) => setName(event.target.value)} placeholder="stitched" aria-label={t('options.name')} />
            </Field>
          </div>
          {format === 'jpeg' && (
            <Field label={t('options.quality')}>
              <NumberInput value={quality} onValueChange={setQuality} min={1} max={100} aria-label={t('options.quality')} />
            </Field>
          )}

          {task.running && (
            <div className="space-y-1.5">
              <Progress value={progress} />
              <p className="text-xs text-fg-muted tabular-nums">
                {t(`stages.${task.stage ?? 'stitch.decode'}`)}
              </p>
            </div>
          )}
          {task.running ? (
            <Button block onClick={task.cancel}>
              <Square />
              {t('files.cancel')}
            </Button>
          ) : (
            <Button block variant="primary" disabled={usable.length < 2 || adding} onClick={start}>
              <Play />
              {t('files.start')}
            </Button>
          )}
        </Panel>
      </div>

      <div className="grid min-h-0 grid-rows-[minmax(0,1fr)_minmax(160px,0.5fr)] gap-3">
        <Panel
          icon={<Images />}
          title={t('result.title')}
          extra={
            task.status === 'succeeded' && report ? (
              <Badge variant="success">{t('result.summary', { width: report.width, height: report.height })}</Badge>
            ) : task.status === 'cancelled' ? (
              <Badge>{t('result.cancelled')}</Badge>
            ) : null
          }
          footer={
            report && (
              <>
                <span className="tabular-nums">{`${formatBytes(report.bytes)} · ${t('result.elapsed', { ms: report.elapsedMs })}`}</span>
                <Button size="sm" variant="outline" onClick={() => run(() => host.revealPath(report.output))}>
                  <FolderSearch />
                  {t('result.reveal')}
                </Button>
              </>
            )
          }
          bodyClassName="flex items-center justify-center overflow-auto bg-surface-2 p-3"
        >
          {task.status === 'failed' && task.error ? (
            <p className="p-4 text-[13px] text-danger">{errorMessage(task.error)}</p>
          ) : report ? (
            <img src={report.thumbnail} alt="" className="max-h-full max-w-full object-contain" />
          ) : task.running ? (
            <div className="flex flex-col items-center gap-2 text-fg-muted">
              <Spinner className="size-6" />
              <p className="text-xs">{t('result.running')}</p>
            </div>
          ) : (
            <Empty icon={<Images />} title={t('result.empty')} description={t('result.emptyHint')} />
          )}
        </Panel>
        <Panel icon={<ScrollText />} title={t('logs.title')} extra={task.logs.length > 0 && <Badge>{task.logs.length}</Badge>}>
          <LogViewer entries={task.logs} emptyText={t('logs.empty')} jumpLabel={t('logs.jumpLatest')} />
        </Panel>
      </div>
    </div>
  )
}
