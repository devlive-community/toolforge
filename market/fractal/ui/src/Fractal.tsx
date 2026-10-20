import { useCallback, useEffect, useRef, useState } from 'react'
import {
  Badge,
  Button,
  Empty,
  NumberInput,
  Panel,
  SegmentedControl,
  Spinner,
  Tooltip,
  cn,
  toast,
} from '@toolforge/ui'
import { host, usePlugin, usePluginState, useTask } from '@toolforge/plugin-ui-sdk'
import {
  Bookmark,
  BookmarkPlus,
  Compass,
  Download,
  Redo2,
  RotateCcw,
  Square,
  Trash2,
  Undo2,
} from 'lucide-react'
import {
  DEFAULT_VIEW,
  MAX_EXPORT_PIXELS,
  MAX_ITER,
  MAX_SPAN,
  MIN_SPAN,
  PALETTES,
  PREVIEW_HEIGHT,
  PREVIEW_WIDTH,
  type Brightness,
  type ExportReport,
  type Favorite,
  type FractalKind,
  type PaletteName,
  type PreviewOut,
  type PreviewStats,
  type Project,
  type View,
} from './types'

const clampSpan = (span: number) => Math.min(Math.max(span, MIN_SPAN), MAX_SPAN)

/** 视角宽度的紧凑显示：常规用定点，深缩放用指数 */
const formatSpan = (span: number) =>
  span >= 0.001 ? String(Math.round(span * 1000) / 1000) : span.toExponential(1)

/** 连续滚轮合并为一步撤销的时间窗 */
const ZOOM_HISTORY_WINDOW_MS = 500

interface ComplexPoint {
  x: number
  y: number
}

export function Fractal() {
  const { t, call, errorMessage } = usePlugin()
  const task = useTask<ExportReport>()
  const [project, setProject] = usePluginState<Project>('project', {
    view: { ...DEFAULT_VIEW },
    favorites: [],
  })
  const [preview, setPreview] = useState<string | null>(null)
  const [stats, setStats] = useState<PreviewStats | null>(null)
  const [busy, setBusy] = useState(false)
  const [exportSize, setExportSize] = useState({ width: 1600, height: 1000 })
  const [panning, setPanning] = useState(false)
  const [historyLen, setHistoryLen] = useState({ undo: 0, redo: 0 })

  const canvasRef = useRef<HTMLCanvasElement | null>(null)
  const seqRef = useRef(0)
  const lastZoomPushRef = useRef(0)
  const panRef = useRef<{
    startX: number
    startY: number
    centerX: number
    centerY: number
    view: View
  } | null>(null)
  const undoRef = useRef<View[]>([])
  const redoRef = useRef<View[]>([])

  const view = project.view

  // 供非 React 事件（原生 wheel / 拖动收尾）读取最新视角；effect 内同步避免渲染期写 ref
  const viewRef = useRef(view)
  useEffect(() => {
    viewRef.current = view
  }, [view])

  const applyView = useCallback(
    (next: View) => {
      setProject((prev) => ({ ...prev, view: next }))
    },
    [setProject],
  )

  const patchView = useCallback(
    (part: Partial<View>) => {
      setProject((prev) => ({ ...prev, view: { ...prev.view, ...part } }))
    },
    [setProject],
  )

  /** 推入撤销栈（记录变更前的视角） */
  const pushHistory = useCallback((snapshot: View) => {
    undoRef.current.push({ ...snapshot })
    redoRef.current = []
    setHistoryLen({ undo: undoRef.current.length, redo: 0 })
  }, [])

  const undo = () => {
    const previous = undoRef.current.pop()
    if (!previous) return
    redoRef.current.push(view)
    setHistoryLen({ undo: undoRef.current.length, redo: redoRef.current.length })
    applyView(previous)
  }

  const redo = () => {
    const next = redoRef.current.pop()
    if (!next) return
    undoRef.current.push(view)
    setHistoryLen({ undo: undoRef.current.length, redo: redoRef.current.length })
    applyView(next)
  }

  /** 重置几何视角，保留配色等风格参数 */
  const resetView = () => {
    pushHistory(view)
    applyView({
      ...view,
      fractal: DEFAULT_VIEW.fractal,
      centerX: DEFAULT_VIEW.centerX,
      centerY: DEFAULT_VIEW.centerY,
      spanX: DEFAULT_VIEW.spanX,
    })
  }

  const switchKind = (kind: FractalKind) => {
    if (kind === view.fractal) return
    pushHistory(view)
    patchView({ fractal: kind })
  }

  // 视角 / 参数变化后防抖重绘预览；序号保证只有最新一次请求生效
  useEffect(() => {
    const timer = setTimeout(() => {
      void (async () => {
        const seq = ++seqRef.current
        setBusy(true)
        try {
          const out = await call<PreviewOut>('preview', {
            ...view,
            width: PREVIEW_WIDTH,
            height: PREVIEW_HEIGHT,
          })
          if (seq !== seqRef.current) return
          setPreview(out.image)
          setStats({
            interiorRatio: out.interiorRatio,
            precisionLimited: out.precisionLimited,
            elapsedMs: out.elapsedMs,
          })
        } catch (error) {
          if (seq !== seqRef.current) return
          setPreview(null)
          toast.error(errorMessage(error))
        } finally {
          if (seq === seqRef.current) setBusy(false)
        }
      })()
    }, 90)
    return () => clearTimeout(timer)
  }, [call, errorMessage, view])

  // 预览帧绘制到画布
  useEffect(() => {
    if (!preview) return
    const timer = setTimeout(() => {
      const image = new Image()
      image.onload = () => {
        canvasRef.current?.getContext('2d')?.drawImage(image, 0, 0)
      }
      image.src = preview
    }, 0)
    return () => clearTimeout(timer)
  }, [preview])

  /** 显示坐标 → 复平面坐标（画布 object-contain 留白时同样成立） */
  const clientToComplex = (clientX: number, clientY: number, base: View): ComplexPoint | null => {
    const canvas = canvasRef.current
    if (!canvas) return null
    const rect = canvas.getBoundingClientRect()
    const scale = Math.min(rect.width / PREVIEW_WIDTH, rect.height / PREVIEW_HEIGHT)
    const offsetX = (rect.width - PREVIEW_WIDTH * scale) / 2
    const offsetY = (rect.height - PREVIEW_HEIGHT * scale) / 2
    const px = (clientX - rect.left - offsetX) / scale
    const py = (clientY - rect.top - offsetY) / scale
    const spanY = (base.spanX * PREVIEW_HEIGHT) / PREVIEW_WIDTH
    return {
      x: base.centerX - base.spanX / 2 + ((px + 0.5) / PREVIEW_WIDTH) * base.spanX,
      y: base.centerY + spanY / 2 - ((py + 0.5) / PREVIEW_HEIGHT) * spanY,
    }
  }

  // 滚轮缩放：以指针为锚点保持指向不变；React 的 onWheel 是 passive，改用原生监听
  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const onWheel = (event: WheelEvent) => {
      event.preventDefault()
      const base = viewRef.current
      const point = clientToComplex(event.clientX, event.clientY, base)
      if (!point) return
      const factor = event.deltaY < 0 ? 0.8 : 1.25
      const span = clampSpan(base.spanX * factor)
      const ratio = span / base.spanX
      const now = Date.now()
      if (now - lastZoomPushRef.current > ZOOM_HISTORY_WINDOW_MS) {
        pushHistory(base)
        lastZoomPushRef.current = now
      }
      patchView({
        spanX: span,
        centerX: point.x + (base.centerX - point.x) * ratio,
        centerY: point.y + (base.centerY - point.y) * ratio,
      })
    }
    canvas.addEventListener('wheel', onWheel, { passive: false })
    return () => canvas.removeEventListener('wheel', onWheel)
  }, [patchView, pushHistory])

  const onMouseDown = (event: React.MouseEvent) => {
    const point = clientToComplex(event.clientX, event.clientY, view)
    if (!point) return
    panRef.current = {
      startX: point.x,
      startY: point.y,
      centerX: view.centerX,
      centerY: view.centerY,
      view,
    }
    setPanning(true)
  }

  const onMouseMove = (event: React.MouseEvent) => {
    const pan = panRef.current
    if (!pan) return
    const point = clientToComplex(event.clientX, event.clientY, pan.view)
    if (!point) return
    patchView({
      centerX: pan.centerX - (point.x - pan.startX),
      centerY: pan.centerY - (point.y - pan.startY),
    })
  }

  const endPan = () => {
    const pan = panRef.current
    panRef.current = null
    setPanning(false)
    if (!pan) return
    const current = viewRef.current
    if (current.centerX !== pan.centerX || current.centerY !== pan.centerY) {
      pushHistory(pan.view)
    }
  }

  /** 双击取点：把点选位置设为 Julia 参数 c（并切换到 Julia） */
  const onDoubleClick = (event: React.MouseEvent) => {
    const point = clientToComplex(event.clientX, event.clientY, view)
    if (!point) return
    pushHistory(view)
    patchView({ fractal: 'julia', juliaX: point.x, juliaY: point.y })
  }

  const addFavorite = () => {
    setProject((prev) => ({
      ...prev,
      favorites: [...prev.favorites, { id: crypto.randomUUID(), view: { ...prev.view } }],
    }))
  }

  const restoreFavorite = (favorite: Favorite) => {
    pushHistory(view)
    applyView({ ...favorite.view })
  }

  const removeFavorite = (id: string) => {
    setProject((prev) => ({ ...prev, favorites: prev.favorites.filter((f) => f.id !== id) }))
  }

  const setExportDim = (key: 'width' | 'height', value: number) => {
    const other = key === 'width' ? exportSize.height : exportSize.width
    if (value * other > MAX_EXPORT_PIXELS) {
      toast.error(t('errors.fractal.pixels_too_large', { limit: '2.5 MP' }))
      setExportSize((prev) => ({ ...prev, [key]: Math.max(1, Math.floor(MAX_EXPORT_PIXELS / other)) }))
      return
    }
    setExportSize((prev) => ({ ...prev, [key]: value }))
  }

  const run = async (action: () => Promise<void>) => {
    try {
      await action()
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const doExport = () =>
    run(async () => {
      const path = await host.dialog.saveFile('fractal.png', [
        { name: 'PNG', extensions: ['png'] },
      ])
      if (!path) return
      task.start('export', { ...view, ...exportSize, outputPath: path })
    })

  const progress =
    task.progress && task.progress.total > 0
      ? (task.progress.done / task.progress.total) * 100
      : null
  const ready = preview !== null

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(300px,0.8fr)_minmax(0,1.4fr)] gap-3">
      <div className="flex min-h-0 flex-col gap-3">
        <Panel
          icon={<Compass />}
          title={t('view.title')}
          className="min-h-0 flex-1"
          bodyClassName="space-y-3.5 overflow-auto p-3"
        >
          <div className="space-y-1.5">
            <p className="text-xs font-medium text-fg-muted">{t('view.kind')}</p>
            <SegmentedControl<FractalKind>
              value={view.fractal}
              onValueChange={switchKind}
              aria-label={t('view.kind')}
              options={[
                { value: 'mandelbrot', label: t('kind.mandelbrot') },
                { value: 'julia', label: t('kind.julia') },
              ]}
            />
          </div>
          {view.fractal === 'julia' && (
            <div className="grid grid-cols-2 gap-2">
              <div className="space-y-1.5">
                <p className="text-xs font-medium text-fg-muted">{t('view.juliaX')}</p>
                <NumberInput
                  value={view.juliaX}
                  onValueChange={(juliaX) => patchView({ juliaX })}
                  step={0.01}
                  aria-label={t('view.juliaX')}
                />
              </div>
              <div className="space-y-1.5">
                <p className="text-xs font-medium text-fg-muted">{t('view.juliaY')}</p>
                <NumberInput
                  value={view.juliaY}
                  onValueChange={(juliaY) => patchView({ juliaY })}
                  step={0.01}
                  aria-label={t('view.juliaY')}
                />
              </div>
            </div>
          )}
          <div className="grid grid-cols-2 gap-2">
            <div className="space-y-1.5">
              <p className="text-xs font-medium text-fg-muted">{t('view.maxIter')}</p>
              <NumberInput
                value={view.maxIter}
                onValueChange={(maxIter) => patchView({ maxIter })}
                min={1}
                max={MAX_ITER}
                aria-label={t('view.maxIter')}
              />
            </div>
            <div className="flex items-end">
              <Button block variant="outline" onClick={resetView}>
                <RotateCcw />
                {t('view.reset')}
              </Button>
            </div>
          </div>
          <div className="space-y-1.5">
            <p className="text-xs font-medium text-fg-muted">{t('view.palette')}</p>
            <SegmentedControl<PaletteName>
              value={view.palette}
              onValueChange={(palette) => patchView({ palette })}
              aria-label={t('view.palette')}
              options={PALETTES.map((name) => ({ value: name, label: t(`palette.${name}`) }))}
            />
          </div>
          <div className="space-y-1.5">
            <p className="text-xs font-medium text-fg-muted">{t('view.brightness')}</p>
            <SegmentedControl<Brightness>
              value={view.brightness}
              onValueChange={(brightness) => patchView({ brightness })}
              aria-label={t('view.brightness')}
              options={[
                { value: 'normal', label: t('brightness.normal') },
                { value: 'bright', label: t('brightness.bright') },
                { value: 'dim', label: t('brightness.dim') },
              ]}
            />
          </div>
          <p className="text-[11px] text-fg-subtle">{t('view.hint')}</p>
        </Panel>

        <Panel
          icon={<Bookmark />}
          title={t('favorites.title')}
          className="shrink-0"
          bodyClassName="p-3"
          actions={
            <Button size="sm" variant="outline" onClick={addFavorite}>
              <BookmarkPlus />
              {t('favorites.add')}
            </Button>
          }
        >
          {project.favorites.length === 0 ? (
            <p className="text-xs text-fg-subtle">{t('favorites.empty')}</p>
          ) : (
            <ul className="max-h-40 space-y-1 overflow-auto">
              {project.favorites.map((favorite) => (
                <li key={favorite.id} className="flex items-center gap-1">
                  <button
                    type="button"
                    onClick={() => restoreFavorite(favorite)}
                    className="min-w-0 flex-1 truncate rounded-control px-2 py-1 text-left text-xs hover:bg-hover"
                  >
                    {t('favorites.item', {
                      kind: t(`kind.${favorite.view.fractal}`),
                      span: formatSpan(favorite.view.spanX),
                    })}
                  </button>
                  <Tooltip content={t('favorites.remove')}>
                    <Button
                      size="icon-sm"
                      variant="ghost"
                      aria-label={t('favorites.remove')}
                      onClick={() => removeFavorite(favorite.id)}
                    >
                      <Trash2 />
                    </Button>
                  </Tooltip>
                </li>
              ))}
            </ul>
          )}
        </Panel>

        <Panel icon={<Download />} title={t('export.title')} className="shrink-0" bodyClassName="space-y-3 p-3">
          <div className="grid grid-cols-2 gap-2">
            <div className="space-y-1.5">
              <p className="text-xs font-medium text-fg-muted">{t('export.width')}</p>
              <NumberInput
                value={exportSize.width}
                onValueChange={(width) => setExportDim('width', width)}
                min={16}
                aria-label={t('export.width')}
              />
            </div>
            <div className="space-y-1.5">
              <p className="text-xs font-medium text-fg-muted">{t('export.height')}</p>
              <NumberInput
                value={exportSize.height}
                onValueChange={(height) => setExportDim('height', height)}
                min={16}
                aria-label={t('export.height')}
              />
            </div>
          </div>
          {task.running && (
            <div className="space-y-1.5">
              <div className="h-1.5 overflow-hidden rounded-full bg-active">
                <div className="h-full rounded-full bg-primary transition-all" style={{ width: `${progress ?? 0}%` }} />
              </div>
              <p className="text-xs text-fg-muted tabular-nums">{t('export.rendering')}</p>
            </div>
          )}
          {task.running ? (
            <Button block onClick={task.cancel}>
              <Square />
              {t('export.cancel')}
            </Button>
          ) : (
            <Button block variant="primary" disabled={!ready} onClick={doExport}>
              <Download />
              PNG
            </Button>
          )}
          {task.result && (
            <p className="text-xs text-fg-muted tabular-nums">
              {t('export.done', {
                width: task.result.width,
                height: task.result.height,
                seconds: (task.result.elapsedMs / 1000).toFixed(1),
              })}
            </p>
          )}
          {task.status === 'failed' && task.error && (
            <p className="text-xs text-danger">{errorMessage(task.error)}</p>
          )}
        </Panel>
      </div>

      <Panel
        icon={<Compass />}
        title={t('canvas.title')}
        extra={
          <div className="flex items-center gap-1.5">
            <Badge variant="info">{t('canvas.span', { span: formatSpan(view.spanX) })}</Badge>
            <Badge>{t('canvas.iter', { iter: view.maxIter })}</Badge>
          </div>
        }
        actions={
          <div className="flex items-center gap-1">
            <Tooltip content={t('history.undo')}>
              <Button
                size="icon-sm"
                variant="ghost"
                aria-label={t('history.undo')}
                disabled={historyLen.undo === 0}
                onClick={undo}
              >
                <Undo2 />
              </Button>
            </Tooltip>
            <Tooltip content={t('history.redo')}>
              <Button
                size="icon-sm"
                variant="ghost"
                aria-label={t('history.redo')}
                disabled={historyLen.redo === 0}
                onClick={redo}
              >
                <Redo2 />
              </Button>
            </Tooltip>
          </div>
        }
        className="min-h-0"
        bodyClassName="relative flex items-center justify-center overflow-auto bg-surface-2 p-3"
        footer={
          <div className="space-y-1">
            <p className="text-xs text-fg-subtle">{t('canvas.hint')}</p>
            <p className="text-xs text-fg-subtle">{t('status.legend', { iter: view.maxIter })}</p>
            {stats?.precisionLimited && <p className="text-xs text-danger">{t('status.precision')}</p>}
          </div>
        }
      >
        <canvas
          ref={canvasRef}
          width={PREVIEW_WIDTH}
          height={PREVIEW_HEIGHT}
          onMouseDown={onMouseDown}
          onMouseMove={onMouseMove}
          onMouseUp={endPan}
          onMouseLeave={endPan}
          onDoubleClick={onDoubleClick}
          className={cn(
            'max-h-full max-w-full rounded-card bg-black shadow-card',
            panning ? 'cursor-grabbing' : 'cursor-grab',
          )}
          aria-label={t('canvas.title')}
        />
        {!ready && (
          <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
            {busy ? (
              <Spinner className="size-6 text-fg-subtle" />
            ) : (
              <Empty
                icon={<Compass />}
                title={t('canvas.empty')}
                description={t('canvas.emptyHint')}
              />
            )}
          </div>
        )}
        {busy && ready && (
          <div className="pointer-events-none absolute right-4 top-4">
            <Badge variant="info">{t('status.rendering')}</Badge>
          </div>
        )}
      </Panel>
    </div>
  )
}
