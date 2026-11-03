import { useCallback, useEffect, useRef, useState } from 'react'
import { Button, Panel, SegmentedControl } from '@toolforge/ui'
import { callPlugin, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Download, Eraser, Redo2, Undo2 } from 'lucide-react'

interface Stroke {
  points: number[]
  color: string
  width: number
  opacity: number
  mode: string
  folds: number
  center: [number, number]
}

const MODES = ['none', 'mirror_x', 'mirror_y', 'mirror_xy', 'rotate'] as const
type Mode = (typeof MODES)[number]
const COLORS = ['#0f172a', '#db2777', '#2563eb', '#16a34a', '#ea580c'] // tf-allow: 画笔配色

export default function SymmetryDrawing() {
  const { t } = usePlugin()
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const [mode, setMode] = useState<Mode>('rotate')
  const [folds, setFolds] = useState(6)
  const [color, setColor] = useState(COLORS[0]) // tf-allow: 默认值
  const [lineWidth, setLineWidth] = useState(3) // tf-allow: 默认值
  const [center, setCenter] = useState<[number, number]>([200, 200])
  const [showGuides, setShowGuides] = useState(true)
  const strokesRef = useRef<Stroke[]>([])
  const redoRef = useRef<Stroke[][]>([])
  const drawingRef = useRef(false)
  const [count, setCount] = useState(0)

  const SIZE = 400

  /** 在 canvas 上画一条笔画的全部对称副本（与 Rust 导出一致的几何） */
  const paintStroke = useCallback((context: CanvasRenderingContext2D, stroke: Stroke) => {
    const points: [number, number][] = []
    for (let index = 0; index < stroke.points.length; index += 2) {
      points.push([stroke.points[index], stroke.points[index + 1]])
    }
    const [cx, cy] = stroke.center
    const copies: [number, number][][] = [points]
    if (stroke.mode === 'mirror_x') copies.push(points.map(([x, y]) => [x, 2 * cy - y]))
    if (stroke.mode === 'mirror_y') copies.push(points.map(([x, y]) => [2 * cx - x, y]))
    if (stroke.mode === 'mirror_xy') {
      copies.push(points.map(([x, y]) => [2 * cx - x, y]))
      copies.push(points.map(([x, y]) => [x, 2 * cy - y]))
      copies.push(points.map(([x, y]) => [2 * cx - x, 2 * cy - y]))
    }
    if (stroke.mode === 'rotate') {
      const angle = (Math.PI * 2) / stroke.folds
      for (let fold = 1; fold < stroke.folds; fold++) {
        const theta = angle * fold
        const sin = Math.sin(theta)
        const cos = Math.cos(theta)
        copies.push(points.map(([x, y]) => {
          const dx = x - cx
          const dy = y - cy
          return [cx + dx * cos - dy * sin, cy + dx * sin + dy * cos]
        }))
      }
    }
    context.globalAlpha = stroke.opacity
    context.strokeStyle = stroke.color
    context.lineWidth = stroke.width
    context.lineCap = 'round'
    context.lineJoin = 'round'
    for (const copy of copies) {
      context.beginPath()
      copy.forEach(([x, y], index) => {
        if (index === 0) context.moveTo(x, y)
        else context.lineTo(x, y)
      })
      context.stroke()
    }
    context.globalAlpha = 1
  }, [])

  const repaint = useCallback((strokes: Stroke[]) => {
    const canvas = canvasRef.current
    const context = canvas?.getContext('2d')
    if (!canvas || !context) return
    context.clearRect(0, 0, SIZE, SIZE)
    if (showGuides) {
      context.strokeStyle = 'rgba(148,163,184,0.35)'
      context.setLineDash([4, 4])
      context.beginPath()
      context.arc(center[0], center[1], 60, 0, Math.PI * 2)
      context.stroke()
      context.setLineDash([])
    }
    for (const stroke of strokes) paintStroke(context, stroke)
  }, [center, showGuides, paintStroke])

  useEffect(() => {
    repaint(strokesRef.current)
  }, [repaint, showGuides, center])

  const getPoint = (event: React.PointerEvent<HTMLCanvasElement>): [number, number] => {
    const rect = event.currentTarget.getBoundingClientRect()
    const scale = SIZE / rect.width
    return [(event.clientX - rect.left) * scale, (event.clientY - rect.top) * scale]
  }

  const onDoubleClick = (event: React.PointerEvent<HTMLCanvasElement>) => {
    setCenter(getPoint(event))
  }

  const onPointerDown = (event: React.PointerEvent<HTMLCanvasElement>) => {
    drawingRef.current = true
    const [x, y] = getPoint(event)
    strokesRef.current.push({
      points: [x, y], color, width: lineWidth, opacity: 1, mode, folds, center,
    })
    redoRef.current = []
  }

  const onPointerMove = (event: React.PointerEvent<HTMLCanvasElement>) => {
    if (!drawingRef.current) return
    const [x, y] = getPoint(event)
    const current = strokesRef.current[strokesRef.current.length - 1]
    current.points.push(x, y)
    repaint(strokesRef.current)
    setCount(strokesRef.current.length)
  }

  const onPointerUp = () => {
    drawingRef.current = false
  }

  const undo = () => {
    if (strokesRef.current.length === 0) return
    redoRef.current.push(strokesRef.current.slice())
    strokesRef.current = strokesRef.current.slice(0, -1)
    repaint(strokesRef.current)
    setCount(strokesRef.current.length)
  }

  const redo = () => {
    const next = redoRef.current.pop()
    if (next) {
      strokesRef.current = next
      repaint(next)
      setCount(next.length)
    }
  }

  const clear = () => {
    redoRef.current.push(strokesRef.current.slice())
    strokesRef.current = []
    repaint([])
    setCount(0)
  }

  const exportSvg = async () => {
    const svg = await callPlugin<{ svg: string }>('org.devlive.toolforge.symmetry-drawing', 'export_svg', {
      strokes: strokesRef.current, width: SIZE, height: SIZE, background: 'transparent',
    })
    const blob = new Blob([svg.svg], { type: 'image/svg+xml' })
    const url = URL.createObjectURL(blob)
    const anchor = document.createElement('a')
    anchor.href = url
    anchor.download = 'symmetry.svg'
    anchor.click()
    URL.revokeObjectURL(url)
  }

  const exportPng = () => {
    const canvas = canvasRef.current
    if (!canvas) return
    const anchor = document.createElement('a')
    anchor.href = canvas.toDataURL('image/png')
    anchor.download = 'symmetry.png'
    anchor.click()
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_minmax(240px,280px)] gap-3">
      <Panel title={t('ui.color')} className="min-h-0" bodyClassName="flex items-center justify-center p-3">
        <canvas
          ref={canvasRef}
          width={SIZE}
          height={SIZE}
          className="max-h-full max-w-full cursor-crosshair rounded-card border border-border bg-white"
          onDoubleClick={onDoubleClick}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerLeave={onPointerUp}
          aria-label={t('ui.color')}
        />
      </Panel>
      <div className="flex min-h-0 flex-col gap-3">
        <Panel title={t('ui.center')} bodyClassName="space-y-2.5 p-3">
          <SegmentedControl
            size="sm"
            value={mode}
            onValueChange={(value) => setMode(value as Mode)}
            aria-label={t('ui.center')}
            options={MODES.map((item) => ({ value: item, label: t(`modes.${item}`) }))}
          />
          {mode === 'rotate' && (
            <label className="flex items-center gap-2 text-xs text-fg-muted">
              {t('ui.folds')}
              <input type="range" min={2} max={16} value={folds} onChange={(event) => setFolds(Number(event.target.value))} className="min-w-0 flex-1 accent-primary" aria-label={t('ui.folds')} />
              <span className="w-6 text-right tabular-nums">{folds}</span>
            </label>
          )}
          <div className="flex gap-1.5">
            {COLORS.map((item) => (
              <button
                key={item}
                type="button"
                onClick={() => setColor(item)}
                aria-label={item}
                className={`size-6 rounded-control border-2 outline-none transition-transform focus-visible:ring-2 focus-visible:ring-ring ${color === item ? 'border-fg scale-110' : 'border-transparent'}`}
                style={{ backgroundColor: item }}
              />
            ))}
          </div>
          <label className="flex items-center gap-2 text-xs text-fg-muted">
            {t('ui.width')}
            <input type="range" min={1} max={12} value={lineWidth} onChange={(event) => setLineWidth(Number(event.target.value))} className="min-w-0 flex-1 accent-primary" aria-label={t('ui.width')} />
          </label>
          <label className="flex items-center gap-1.5 text-xs text-fg-muted">
            <input type="checkbox" checked={showGuides} onChange={(event) => setShowGuides(event.target.checked)} className="size-3.5" />
            {t('ui.guides')}
          </label>
        </Panel>
        <Panel title={t('ui.save')} bodyClassName="space-y-2 p-3">
          <div className="flex flex-wrap gap-2">
            <Button size="sm" variant="ghost" onClick={undo} aria-label={t('ui.undo')} disabled={count === 0}>
              <Undo2 className="size-3.5" />
            </Button>
            <Button size="sm" variant="ghost" onClick={redo} aria-label={t('ui.redo')}>
              <Redo2 className="size-3.5" />
            </Button>
            <Button size="sm" variant="ghost" onClick={clear} aria-label={t('ui.clear')}>
              <Eraser className="size-3.5" />
            </Button>
            <span className="ml-auto self-center text-[11px] text-fg-subtle">{t('ui.strokes', { count })}</span>
          </div>
          <div className="flex gap-2">
            <Button size="sm" variant="secondary" onClick={exportPng}>
              <Download className="size-3.5" />
              {t('ui.exportPng')}
            </Button>
            <Button size="sm" variant="secondary" onClick={() => void exportSvg()}>
              {t('ui.exportSvg')}
            </Button>
          </div>
        </Panel>
      </div>
    </div>
  )
}
