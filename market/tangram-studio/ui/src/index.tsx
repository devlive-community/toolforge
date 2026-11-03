import { useCallback, useEffect, useRef, useState } from 'react'
import { Badge, Button, Panel, Select } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { RotateCw, Triangle } from 'lucide-react'

interface Piece {
  kind: string
  index: number
  points: [number, number][]
  area: number
}
interface ValidateOut {
  status: string
  issues: string[]
}

const SCALE = 40 // 4x4 棋盘每单位 40px

export default function TangramStudio() {
  const { t, call } = usePlugin()
  const piecesRef = useRef<Piece[]>([])
  const puzzlesRef = useRef<{ key: string; outline: [number, number][] }[]>([])
  const [catalog, setCatalog] = useState<{ pieces: Piece[]; puzzles: { key: string; outline: [number, number][] }[] } | null>(null)
  const [ready, setReady] = useState(false)
  const [puzzleKey, setPuzzleKey] = useState('square')
  const [selected, setSelected] = useState<number | null>(null)
  const [validate, setValidate] = useState<ValidateOut | null>(null)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const placementsRef = useRef<{ kind: string; index: number; x: number; y: number; rotation: number; flipped: boolean }[]>([])
  const dragRef = useRef<{ piece: number; startX: number; startY: number; baseX: number; baseY: number } | null>(null)

  const outlineFor = useCallback(
    (key: string, puzzles: { key: string; outline: [number, number][] }[]) =>
      puzzles.find((p) => p.key === key)?.outline ?? [],
    [],
  )

  const draw = useCallback(
    (outline: [number, number][], pieces: Piece[], placements: typeof placementsRef.current) => {
      const canvas = canvasRef.current
      const context = canvas?.getContext('2d')
      if (!canvas || !context) return
      context.clearRect(0, 0, canvas.width, canvas.height)
      // 轮廓
      context.beginPath()
      outline.forEach(([x, y], index) => {
        if (index === 0) context.moveTo(x * SCALE, y * SCALE)
        else context.lineTo(x * SCALE, y * SCALE)
      })
      context.closePath()
      context.fillStyle = '#f8fafc'
      context.fill()
      context.strokeStyle = '#334155'
      context.lineWidth = 2
      context.stroke()
      // 拼块
      const catalog = pieces
      for (const placement of placements) {
        const piece = catalog.find((p) => p.kind === placement.kind && p.index === placement.index)
        if (!piece) continue
        const theta = (placement.rotation * Math.PI) / 180
        const sin = Math.sin(theta)
        const cos = Math.cos(theta)
        context.beginPath()
        piece.points.forEach(([x, y], index) => {
          const px = placement.flipped ? -x : x
          const screenX = (placement.x + px * cos - y * sin) * SCALE
          const screenY = (placement.y + px * sin + y * cos) * SCALE
          if (index === 0) context.moveTo(screenX, screenY)
          else context.lineTo(screenX, screenY)
        })
        context.closePath()
        context.fillStyle = placement.kind === 'large' ? '#fbbf24' : placement.kind === 'medium' ? '#fb923c' : placement.kind === 'small' ? '#f87171' : placement.kind === 'square' ? '#a78bfa' : '#60a5fa'
        context.fill()
        context.strokeStyle = '#111827'
        context.lineWidth = 1
        context.stroke()
      }
    },
    [],
  )

  const hitPiece = (mouseX: number, mouseY: number) => {
    // 从上往下找命中拼块（近似：中心距离）
    let best: number | null = null
    let bestDist = Infinity
    placementsRef.current.forEach((placement, index) => {
      const dx = (placement.x + 1) * SCALE - mouseX
      const dy = (placement.y + 1) * SCALE - mouseY
      const dist = dx * dx + dy * dy
      if (dist < bestDist) {
        bestDist = dist
        best = index
      }
    })
    return best
  }

  const onPointerDown = (event: React.PointerEvent<HTMLCanvasElement>) => {
    const rect = event.currentTarget.getBoundingClientRect()
    const mouseX = ((event.clientX - rect.left) / rect.width) * (canvasRef.current?.width ?? 320)
    const mouseY = ((event.clientY - rect.top) / rect.height) * (canvasRef.current?.height ?? 320)
    const hit = hitPiece(mouseX, mouseY)
    if (hit !== null) {
      setSelected(hit)
      dragRef.current = { piece: hit, startX: mouseX, startY: mouseY, baseX: placementsRef.current[hit].x, baseY: placementsRef.current[hit].y }
    }
  }

  const onPointerMove = (event: React.PointerEvent<HTMLCanvasElement>) => {
    if (!dragRef.current) return
    const rect = event.currentTarget.getBoundingClientRect()
    const mouseX = ((event.clientX - rect.left) / rect.width) * (canvasRef.current?.width ?? 320)
    const mouseY = ((event.clientY - rect.top) / rect.height) * (canvasRef.current?.height ?? 320)
    const drag = dragRef.current
    placementsRef.current[drag.piece].x = drag.baseX + (mouseX - drag.startX) / SCALE
    placementsRef.current[drag.piece].y = drag.baseY + (mouseY - drag.startY) / SCALE
    const outline = outlineFor(puzzleKey, puzzlesRef.current)
    draw(outline, piecesRef.current, placementsRef.current)
  }

  const rotateSelected = () => {
    if (selected === null) return
    placementsRef.current[selected].rotation = (placementsRef.current[selected].rotation + 45) % 360
    const outline = outlineFor(puzzleKey, puzzlesRef.current)
    draw(outline, piecesRef.current, placementsRef.current)
  }

  const flipSelected = () => {
    if (selected === null) return
    placementsRef.current[selected].flipped = !placementsRef.current[selected].flipped
    const outline = outlineFor(puzzleKey, puzzlesRef.current)
    draw(outline, piecesRef.current, placementsRef.current)
  }

  const check = async () => {
    try {
      const outline = outlineFor(puzzleKey, puzzlesRef.current)
      const flat: number[] = []
      for (const [x, y] of outline) {
        flat.push(x, y)
      }
      const out = await call<ValidateOut>('validate', { outline: flat, placements: placementsRef.current })
      setValidate(out)
    } catch {
      setValidate(null)
    }
  }

  useEffect(() => {
    let cancelled = false
    void (async () => {
      const pieceList = await call<Piece[]>('pieces', {})
      const puzzleList = await call<{ key: string; outline: [number, number][] }[]>('puzzles', {})
      if (cancelled) return
      // 用 rAF 推迟 setState，避免 effect 级联渲染告警
      requestAnimationFrame(() => {
        if (cancelled) return
        placementsRef.current = pieceList.map((piece) => ({ kind: piece.kind, index: piece.index, x: 0, y: 0, rotation: 0, flipped: false }))
        setCatalog({ pieces: pieceList, puzzles: puzzleList })
        setReady(true)
      })
    })()
    return () => {
      cancelled = true
    }
  }, [call])

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_minmax(220px,260px)] gap-3">
      <Panel icon={<Triangle />} title={t('ui.svg')} className="min-h-0" bodyClassName="flex items-center justify-center p-3" >
        {ready && (
        <canvas
          ref={canvasRef}
          width={320}
          height={320}
          className="max-h-full cursor-move rounded-card border border-border bg-surface"
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={() => (dragRef.current = null)}
          onPointerLeave={() => (dragRef.current = null)}
          aria-label={t('ui.svg')}
        />
        )}
      </Panel>
      <div className="flex min-h-0 flex-col gap-3">
        <Panel icon={<Triangle />} title={t('ui.puzzle')} bodyClassName="space-y-2 p-3">
          <Select value={puzzleKey} onValueChange={setPuzzleKey} options={(catalog?.puzzles ?? []).map((p) => ({ value: p.key, label: p.key }))} aria-label={t('ui.puzzle')} />
          <div className="flex flex-wrap gap-2">
            <Button size="sm" variant="secondary" onClick={rotateSelected} disabled={selected === null}>
              <RotateCw className="size-3.5" />
              {t('ui.rotate')}
            </Button>
            <Button size="sm" variant="secondary" onClick={flipSelected} disabled={selected === null}>
              {t('ui.flip')}
            </Button>
            <Button size="sm" onClick={() => void check()}>
              {t('ui.check')}
            </Button>
          </div>
                {validate && (
            <Badge variant={validate.status === 'complete' ? 'success' : 'danger'}>{t(`status.${validate.status}`)}</Badge>
          )}
              {selected !== null && catalog?.pieces[selected] && (
            <p className="text-[11px] text-fg-subtle">{catalog?.pieces[selected].kind} #{catalog?.pieces[selected].index}</p>
          )}
        </Panel>
      </div>
    </div>
  )
}
