import { useEffect, useRef } from 'react'
import { frameSrc } from './store'
import type { Tool } from './types'

/** 一笔的输入（画布像素坐标），抬笔后交给 Rust 栅格化 */
export interface StrokeInput {
  tool: Tool
  color: string
  size: number
  points: [number, number][]
}

interface StageProps {
  width: number
  height: number
  /** 当前显示的帧内容（播放时为播放到的帧） */
  content: string | undefined
  /** 洋葱皮：前一帧内容，只在画画时半透明显示，不进入任何导出 */
  onion: string | undefined
  tool: Tool
  color: string
  size: number
  /** 播放或导出中不能画 */
  disabled: boolean
  /** 已抬笔但还没写入帧的笔画：先画在预览层，写入成功后由调用方移除 */
  pending: StrokeInput[]
  onStroke: (stroke: StrokeInput) => void
  label: string
}

/** 预览层上画一笔（只是示意，真正的像素由 Rust 生成） */
function drawPreview(ctx: CanvasRenderingContext2D, stroke: StrokeInput) {
  ctx.save()
  ctx.lineCap = 'round'
  ctx.lineJoin = 'round'
  ctx.lineWidth = stroke.size
  // 橡皮在预览层用半透明灰色示意擦除范围
  ctx.strokeStyle = stroke.tool === 'eraser' ? 'rgba(128, 128, 128, 0.45)' : stroke.color
  ctx.fillStyle = ctx.strokeStyle
  const [first, ...rest] = stroke.points
  if (rest.length === 0) {
    ctx.beginPath()
    ctx.arc(first[0], first[1], stroke.size / 2, 0, Math.PI * 2)
    ctx.fill()
  } else {
    ctx.beginPath()
    ctx.moveTo(first[0], first[1])
    for (const [x, y] of rest) ctx.lineTo(x, y)
    ctx.stroke()
  }
  ctx.restore()
}

export function Stage({ width, height, content, onion, tool, color, size, disabled, pending, onStroke, label }: StageProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null)
  const drawing = useRef<StrokeInput | null>(null)

  const redraw = () => {
    const ctx = canvasRef.current?.getContext('2d')
    if (!ctx) return
    ctx.clearRect(0, 0, width, height)
    for (const stroke of pending) drawPreview(ctx, stroke)
    if (drawing.current) drawPreview(ctx, drawing.current)
  }

  // 待写入的笔画变化（写入完成被移除）后重画预览层
  useEffect(redraw)

  const point = (event: React.PointerEvent<HTMLCanvasElement>): [number, number] => {
    const rect = event.currentTarget.getBoundingClientRect()
    return [((event.clientX - rect.left) * width) / rect.width, ((event.clientY - rect.top) * height) / rect.height]
  }

  const onPointerDown = (event: React.PointerEvent<HTMLCanvasElement>) => {
    if (disabled || event.button !== 0) return
    event.currentTarget.setPointerCapture(event.pointerId)
    drawing.current = { tool, color, size, points: [point(event)] }
    redraw()
  }

  const onPointerMove = (event: React.PointerEvent<HTMLCanvasElement>) => {
    const stroke = drawing.current
    if (!stroke) return
    const next = point(event)
    const last = stroke.points[stroke.points.length - 1]
    // 与上一点距离不足半像素的移动不记录，控制点数
    if (Math.hypot(next[0] - last[0], next[1] - last[1]) < 0.5) return
    stroke.points.push(next)
    redraw()
  }

  const finish = () => {
    const stroke = drawing.current
    drawing.current = null
    if (stroke) onStroke(stroke)
  }

  const src = frameSrc(content)
  const onionSrc = frameSrc(onion)
  return (
    <div
      className="relative mx-auto w-full max-w-[640px] overflow-hidden rounded-card border border-border"
      style={{
        aspectRatio: `${width} / ${height}`,
        // 棋盘格表示透明区域（颜色取自主题 token）
        backgroundColor: 'var(--color-surface)',
        backgroundImage:
          'linear-gradient(45deg, var(--color-hover) 25%, transparent 25%), linear-gradient(-45deg, var(--color-hover) 25%, transparent 25%), linear-gradient(45deg, transparent 75%, var(--color-hover) 75%), linear-gradient(-45deg, transparent 75%, var(--color-hover) 75%)',
        backgroundSize: '16px 16px',
        backgroundPosition: '0 0, 0 8px, 8px -8px, -8px 0',
      }}
    >
      {onionSrc && <img src={onionSrc} alt="" draggable={false} className="pointer-events-none absolute inset-0 h-full w-full opacity-30" />}
      {src && <img src={src} alt="" draggable={false} className="pointer-events-none absolute inset-0 h-full w-full" />}
      <canvas
        ref={canvasRef}
        width={width}
        height={height}
        aria-label={label}
        role="img"
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={finish}
        onPointerCancel={finish}
        className={`absolute inset-0 h-full w-full touch-none ${disabled ? 'cursor-not-allowed' : 'cursor-crosshair'}`}
      />
    </div>
  )
}
