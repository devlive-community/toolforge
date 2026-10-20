import { useCallback, useEffect, useRef, useState } from 'react'
import { Badge, Button, Empty, NumberInput, Panel, SegmentedControl, Spinner, Tooltip, cn, toast } from '@toolforge/ui'
import { CopyButton, host, usePlugin, usePluginState, useTask } from '@toolforge/plugin-ui-sdk'
import { Anchor, Download, FileImage, FilePlus, Move, Redo2, Settings2, Sparkles, Square, Trash2, Undo2, Wand2 } from 'lucide-react'
import { DEFAULT_PROJECT, MAX_POINTS, MAX_SCALE, type Mesh, type Project, type ViewMode } from './types'

const IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp', 'tif', 'tiff']

/** 命中半径（画布像素）与显示缩放无关 */
const HIT_RADIUS = 6

interface CanvasBox {
  canvas: HTMLCanvasElement | null
  width: number
  height: number
}

/** 画布坐标 → 工作图坐标 */
function toWork(box: CanvasBox, clientX: number, clientY: number): [number, number] | null {
  const canvas = box.canvas
  if (!canvas) return null
  const rect = canvas.getBoundingClientRect()
  const x = ((clientX - rect.left) / rect.width) * box.width
  const y = ((clientY - rect.top) / rect.height) * box.height
  return [x, y]
}

export function LowPoly() {
  const { t, call, errorMessage } = usePlugin()
  const task = useTask<{ output: string; width: number; height: number; triangles: number; bytes: number }>()
  const [project, setProject] = usePluginState<Project>('project', { ...DEFAULT_PROJECT, workData: '', width: 0, height: 0 })
  const [mesh, setMesh] = useState<Mesh | null>(null)
  const [view, setView] = useState<ViewMode>('result')
  const [busy, setBusy] = useState(false)
  const [workImg, setWorkImg] = useState<HTMLImageElement | null>(null)
  // 撤销 / 重做：点位快照栈
  const undoStack = useRef<[number, number][][]>([])
  const redoStack = useRef<[number, number][][]>([])
  const [historyLen, setHistoryLen] = useState({ undo: 0, redo: 0 })
  const dragging = useRef<number | null>(null)
  const canvasBox = useRef<CanvasBox>({ canvas: null, width: project.width, height: project.height })

  const patch = (value: Partial<Project>) => setProject((prev) => ({ ...prev, ...value }))

  const run = async (action: () => Promise<void>) => {
    setBusy(true)
    try {
      await action()
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setBusy(false)
    }
  }

  /** 工作图加载为 Image 供 canvas 绘制（异步 setTimeout 触发，避免 effect 内同步 setState） */
  useEffect(() => {
    if (!project.workData) {
      setTimeout(() => setWorkImg(null), 0)
      return
    }
    const timer = setTimeout(() => {
      const image = new Image()
      image.onload = () => setWorkImg(image)
      image.src = project.workData
    }, 0)
    return () => clearTimeout(timer)
  }, [project.workData])

  /** 剖分 + 取色（手动点或自动点共用） */
  const remesh = useCallback(
    async (points: [number, number][], brightness: number) => {
      if (!project.workData || points.length < 3) return
      const next = await call<Mesh>('remesh', {
        image: { data: project.workData },
        points,
        brightness,
      })
      setMesh(next)
    },
    [call, project.workData],
  )

  /** 自动生成：种子/数量/细节变化后重新生成点集并剖分 */
  const regenerate = () =>
    run(async () => {
      if (!project.workData) return
      const mesh = await call<Mesh>('generate', {
        image: { data: project.workData },
        seed: project.seed,
        count: project.count,
        detail: project.detail,
      })
      setMesh(mesh)
      setProject((prev) => ({ ...prev, points: mesh.points, auto: true }))
      undoStack.current = []
      redoStack.current = []
      setHistoryLen({ undo: 0, redo: 0 })
      setView('result')
    })

  const pickFile = () =>
    run(async () => {
      const path = await host.dialog.openFile([{ name: t('file.filter'), extensions: IMAGE_EXTENSIONS }])
      if (!path) return
      const out = await call<{ width: number; height: number; workData: string }>('prepare', { path })
      setMesh(null)
      setProject((prev) => ({
        ...prev,
        workData: out.workData,
        width: out.width,
        height: out.height,
        points: [],
        auto: true,
      }))
      undoStack.current = []
      redoStack.current = []
      setHistoryLen({ undo: 0, redo: 0 })
      // 加载后自动出一版
      const mesh = await call<Mesh>('generate', {
        image: { data: out.workData },
        seed: project.seed,
        count: project.count,
        detail: project.detail,
      })
      setMesh(mesh)
      setProject((prev) => ({ ...prev, points: mesh.points, auto: true }))
    })

  /** 推入撤销栈（一次拖动/删除只算一步） */
  const pushHistory = (points: [number, number][]) => {
    undoStack.current.push(points.map((p) => [...p] as [number, number]))
    redoStack.current = []
    setHistoryLen({ undo: undoStack.current.length, redo: 0 })
  }

  const applyPoints = (points: [number, number][], auto = false) => {
    patch({ points, auto })
    void remesh(points, project.brightness)
  }

  const undo = () => {
    const previous = undoStack.current.pop()
    if (!previous) return
    redoStack.current.push(project.points.map((p) => [...p] as [number, number]))
    setHistoryLen({ undo: undoStack.current.length, redo: redoStack.current.length })
    applyPoints(previous)
  }

  const redo = () => {
    const next = redoStack.current.pop()
    if (!next) return
    undoStack.current.push(project.points.map((p) => [...p] as [number, number]))
    setHistoryLen({ undo: undoStack.current.length, redo: redoStack.current.length })
    applyPoints(next)
  }

  const addPoint = (point: [number, number]) => {
    if (project.points.length >= MAX_POINTS) {
      toast.error(t('errors.lowpoly.too_many_points'))
      return
    }
    pushHistory(project.points)
    applyPoints([...project.points, point])
  }

  /** 双击普通点删除（锚点不可删） */
  const onDoubleClick = (event: React.MouseEvent) => {
    if (view !== 'edit' || busy) return
    const index = hitIndex(event.clientX, event.clientY)
    if (index === null) return
    if (index < 4) {
      toast.error(t('hint.anchorFixed'))
      return
    }
    pushHistory(project.points)
    applyPoints(project.points.filter((_, i) => i !== index))
  }

  /** 导出 PNG/SVG（任务化，可取消） */
  const doExport = (format: 'png' | 'svg') =>
    run(async () => {
      const ext = format === 'png' ? 'png' : 'svg'
      const path = await host.dialog.saveFile(`low-poly.${ext}`, [
        { name: ext.toUpperCase(), extensions: [ext] },
      ])
      if (!path) return
      task.start('export', {
        image: { data: project.workData },
        points: project.points,
        scale: project.scale,
        brightness: project.brightness,
        strokeWidth: project.strokeWidth,
        strokeColor: project.strokeColor,
        background: project.background,
        format,
        outputPath: path,
      })
    })

  const report = task.result
  const canvasRef = useCallback((node: HTMLCanvasElement | null) => {
    canvasBox.current.canvas = node
  }, [])

  /** 绘制：原图 / 成品 / 编辑叠加 */
  useEffect(() => {
    const canvas = canvasBox.current.canvas
    if (!canvas || !workImg || project.width === 0) return
    canvas.width = project.width
    canvas.height = project.height
    canvasBox.current.width = project.width
    canvasBox.current.height = project.height
    const ctx = canvas.getContext('2d')
    if (!ctx) return
    ctx.clearRect(0, 0, project.width, project.height)
    if (view !== 'result' || !mesh) {
      ctx.drawImage(workImg, 0, 0)
    }
    if (mesh && view !== 'original') {
      for (const [index, tri] of mesh.triangles.entries()) {
        ctx.beginPath()
        ctx.moveTo(mesh.points[tri[0]][0], mesh.points[tri[0]][1])
        ctx.lineTo(mesh.points[tri[1]][0], mesh.points[tri[1]][1])
        ctx.lineTo(mesh.points[tri[2]][0], mesh.points[tri[2]][1])
        ctx.closePath()
        ctx.fillStyle = mesh.colors[index]
        ctx.fill()
        if (project.strokeWidth > 0) {
          ctx.lineWidth = project.strokeWidth
          ctx.strokeStyle = `#${project.strokeColor}`
          ctx.lineJoin = 'round'
          ctx.stroke()
        }
      }
    }
    if (view === 'edit' && project.points.length > 0) {
      // 控制点：锚点方块，普通点圆点
      project.points.forEach((p, i) => {
        ctx.beginPath()
        if (i < 4) {
          ctx.rect(p[0] - 4, p[1] - 4, 8, 8)
        } else {
          ctx.arc(p[0], p[1], 4, 0, Math.PI * 2)
        }
        ctx.fillStyle = i < 4 ? 'rgba(255,255,255,0.9)' : 'rgba(56,189,248,0.9)'
        ctx.fill()
        ctx.lineWidth = 1
        ctx.strokeStyle = 'rgba(0,0,0,0.55)'
        ctx.stroke()
      })
    }
  }, [canvasRef, mesh, project, view, workImg])

  /** 拖动：mousedown 命中 → move 更新 → up 一次 remesh */
  const hitIndex = (clientX: number, clientY: number): number | null => {
    const point = toWork(canvasBox.current, clientX, clientY)
    if (!point) return null
    let bestIndex: number | null = null
    let bestDistance = Number.POSITIVE_INFINITY
    project.points.forEach((p, i) => {
      const dx = p[0] - point[0]
      const dy = p[1] - point[1]
      const distance = dx * dx + dy * dy
      if (distance <= HIT_RADIUS * HIT_RADIUS && distance < bestDistance) {
        bestIndex = i
        bestDistance = distance
      }
    })
    return bestIndex
  }

  const onMouseDown = (event: React.MouseEvent) => {
    if (view !== 'edit' || busy) return
    const index = hitIndex(event.clientX, event.clientY)
    if (index !== null) {
      dragging.current = index
    } else {
      const point = toWork(canvasBox.current, event.clientX, event.clientY)
      if (point) addPoint(point)
    }
  }

  const onMouseMove = (event: React.MouseEvent) => {
    const index = dragging.current
    if (index === null || index < 4) return
    const point = toWork(canvasBox.current, event.clientX, event.clientY)
    if (!point) return
    const next = [...project.points]
    next[index] = [
      Math.min(Math.max(point[0], 0), project.width - 1),
      Math.min(Math.max(point[1], 0), project.height - 1),
    ]
    setProject((prev) => ({ ...prev, points: next }))
  }

  const onMouseUp = () => {
    const index = dragging.current
    dragging.current = null
    if (index === null) return
    pushHistory(project.points)
    void remesh(project.points, project.brightness)
  }

  const ready = !!project.workData && !!mesh
  const progress = task.progress && task.progress.total > 0 ? (task.progress.done / task.progress.total) * 100 : null

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(300px,0.8fr)_minmax(0,1.4fr)] gap-3">
      <div className="flex min-h-0 flex-col gap-3">
        <Panel
          icon={<FileImage />}
          title={t('file.title')}
          actions={
            <>
              <Button size="sm" onClick={pickFile} disabled={busy || task.running}>
                <FilePlus />
                {t('file.open')}
              </Button>
              {project.workData && (
                <CopyButton text={project.workData} label={t('file.copyWork')} variant="outline" />
              )}
            </>
          }
          className="shrink-0"
          bodyClassName="p-3"
        >
          {!project.workData ? (
            <button
              type="button"
              onClick={pickFile}
              className="flex h-24 w-full flex-col items-center justify-center gap-2 rounded-card border-2 border-dashed border-border text-fg-muted outline-none hover:border-border-strong hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring"
            >
              <FileImage className="size-5" />
              <span className="text-[13px]">{t('file.openHint')}</span>
            </button>
          ) : (
            <div className="flex flex-wrap items-center gap-2 text-xs text-fg-muted tabular-nums">
              <Badge variant="info">{t('file.dimensions', { width: project.width, height: project.height })}</Badge>
              <Badge>{t('file.points', { count: project.points.length })}</Badge>
              {mesh && <Badge>{t('file.triangles', { count: mesh.triangles.length })}</Badge>}
            </div>
          )}
        </Panel>

        <Panel icon={<Settings2 />} title={t('options.title')} className="min-h-0 flex-1" bodyClassName="space-y-3.5 overflow-auto p-3">
          <div className="grid grid-cols-3 gap-2">
            <div className="space-y-1.5">
              <p className="text-xs font-medium text-fg-muted">{t('options.count')}</p>
              <NumberInput
                value={project.count}
                onValueChange={(count) => patch({ count })}
                min={4}
                max={MAX_POINTS}
                aria-label={t('options.count')}
              />
            </div>
            <div className="space-y-1.5">
              <p className="text-xs font-medium text-fg-muted">{t('options.detail')}</p>
              <NumberInput
                value={project.detail}
                onValueChange={(detail) => patch({ detail })}
                min={0}
                max={100}
                aria-label={t('options.detail')}
              />
            </div>
            <div className="space-y-1.5">
              <p className="text-xs font-medium text-fg-muted">{t('options.seed')}</p>
              <NumberInput
                value={project.seed}
                onValueChange={(seed) => patch({ seed })}
                min={1}
                aria-label={t('options.seed')}
              />
            </div>
          </div>
          <Button block onClick={regenerate} disabled={busy || !project.workData || task.running}>
            {busy ? <Spinner /> : <Wand2 />}
            {t('options.regenerate')}
          </Button>

          <div className="grid grid-cols-2 gap-2">
            <div className="space-y-1.5">
              <p className="text-xs font-medium text-fg-muted">{t('options.brightness')}</p>
              <NumberInput
                value={project.brightness}
                onValueChange={(brightness) => {
                  patch({ brightness })
                  void remesh(project.points, brightness)
                }}
                min={-100}
                max={100}
                aria-label={t('options.brightness')}
              />
            </div>
            <div className="space-y-1.5">
              <p className="text-xs font-medium text-fg-muted">{t('options.scale')}</p>
              <NumberInput
                value={project.scale}
                onValueChange={(scale) => patch({ scale })}
                min={1}
                max={MAX_SCALE}
                aria-label={t('options.scale')}
              />
            </div>
          </div>
          <div className="grid grid-cols-2 gap-2">
            <div className="space-y-1.5">
              <p className="text-xs font-medium text-fg-muted">{t('options.strokeWidth')}</p>
              <NumberInput
                value={project.strokeWidth}
                onValueChange={(strokeWidth) => patch({ strokeWidth })}
                min={0}
                max={20}
                aria-label={t('options.strokeWidth')}
              />
            </div>
            <div className="space-y-1.5">
              <p className="text-xs font-medium text-fg-muted">{t('options.background')}</p>
              <input
                type="color"
                value={`#${project.background}`}
                onChange={(event) => patch({ background: event.target.value.replace('#', '') })}
                className="h-9 w-full cursor-pointer rounded-control border border-border bg-surface"
                aria-label={t('options.background')}
              />
            </div>
          </div>
          <p className="-mt-1.5 text-[11px] text-fg-subtle">{t('options.hint')}</p>
        </Panel>

        <Panel icon={<Sparkles />} title={t('export.title')} className="shrink-0" bodyClassName="space-y-3 p-3">
          {task.running && (
            <div className="space-y-1.5">
              <div className="h-1.5 overflow-hidden rounded-full bg-active">
                <div className="h-full rounded-full bg-primary transition-all" style={{ width: `${progress ?? 0}%` }} />
              </div>
              <p className="text-xs text-fg-muted tabular-nums">{t(`stages.${task.stage ?? 'lowpoly.sample'}`)}</p>
            </div>
          )}
          {task.running ? (
            <Button block onClick={task.cancel}>
              <Square />
              {t('export.cancel')}
            </Button>
          ) : (
            <div className="grid grid-cols-2 gap-2">
              <Button variant="primary" disabled={!ready} onClick={() => doExport('png')}>
                <Download />
                PNG
              </Button>
              <Button variant="primary" disabled={!ready} onClick={() => doExport('svg')}>
                <Download />
                SVG
              </Button>
            </div>
          )}
          {report && (
            <p className="text-xs text-fg-muted tabular-nums">
              {t('export.done', { width: report.width, height: report.height, triangles: report.triangles })}
            </p>
          )}
          {task.status === 'failed' && task.error && <p className="text-xs text-danger">{errorMessage(task.error)}</p>}
        </Panel>
      </div>

      <Panel
        icon={<Move />}
        title={t('canvas.title')}
        extra={
          ready ? (
            <SegmentedControl<ViewMode>
              size="sm"
              value={view}
              onValueChange={setView}
              aria-label={t('canvas.view')}
              options={[
                { value: 'edit', label: t('view.edit') },
                { value: 'result', label: t('view.result') },
                { value: 'original', label: t('view.original') },
              ]}
            />
          ) : null
        }
        actions={
          view === 'edit' && (
            <div className="flex items-center gap-1">
              <Tooltip content={t('history.undo')}>
                <Button size="icon-sm" variant="ghost" aria-label={t('history.undo')} disabled={historyLen.undo === 0} onClick={undo}>
                  <Undo2 />
                </Button>
              </Tooltip>
              <Tooltip content={t('history.redo')}>
                <Button size="icon-sm" variant="ghost" aria-label={t('history.redo')} disabled={historyLen.redo === 0} onClick={redo}>
                  <Redo2 />
                </Button>
              </Tooltip>
              <Tooltip content={t('history.clear')}>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  aria-label={t('history.clear')}
                  disabled={project.points.length <= 4}
                  onClick={() => {
                    pushHistory(project.points)
                    applyPoints(project.points.slice(0, 4))
                  }}
                >
                  <Trash2 />
                </Button>
              </Tooltip>
            </div>
          )
        }
        className="min-h-0"
        bodyClassName="flex items-center justify-center overflow-auto bg-surface-2 p-3"
        footer={
          view === 'edit' && ready ? (
            <span className="text-xs text-fg-subtle">
              <Anchor className="mr-1 inline size-3" />
              {t('canvas.hint')}
            </span>
          ) : null
        }
      >
        {!project.workData ? (
          <Empty icon={<Sparkles />} title={t('canvas.empty')} description={t('canvas.emptyHint')} />
        ) : !workImg ? (
          <Spinner className="size-6 text-fg-subtle" />
        ) : (
          <canvas
            ref={canvasRef}
            onMouseDown={onMouseDown}
            onMouseMove={onMouseMove}
            onMouseUp={onMouseUp}
            onMouseLeave={onMouseUp}
            onDoubleClick={onDoubleClick}
            className={cn(
              'max-h-full max-w-full bg-white shadow-card',
              view === 'edit' ? 'cursor-crosshair' : 'cursor-default',
            )}
            aria-label={t('canvas.title')}
          />
        )}
      </Panel>
    </div>
  )
}
