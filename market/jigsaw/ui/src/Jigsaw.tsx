import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import {
  Badge,
  Button,
  Empty,
  Panel,
  SegmentedControl,
  Spinner,
  Tooltip,
  toast,
} from '@toolforge/ui'
import { host, usePlugin, usePluginState, useTask } from '@toolforge/plugin-ui-sdk'
import {
  Bookmark,
  Eye,
  EyeOff,
  ImagePlus,
  Lightbulb,
  Play,
  Puzzle,
  Shuffle,
  Trash2,
  Trophy,
} from 'lucide-react'
import {
  formatClock,
  outlineBBox,
  outlineToPath,
  pieceCategory,
  pieceOutline,
  type Crop,
  type GameData,
  type GameSetup,
  type PieceFilter,
  type PieceState,
  type PlaceOut,
  type Prepared,
  type RecordEntry,
  type VerifyOut,
  GRIDS,
  MAX_RECORDS,
  MAX_SAVES,
} from './pieces'

const IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp']
const MIN_CROP = 90
const ZOOM_MIN = 0.05
const ZOOM_MAX = 4
const DRAG_TOLERANCE_RATIO = 0.3

interface Camera {
  z: number
  cx: number
  cy: number
}

interface SavedGame {
  game: GameData
  ms: number
  savedAt: number
}

interface DragState {
  /** pieces 数组中的当前位置 */
  index: number
  offX: number
  offY: number
  x: number
  y: number
}

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max)
}

export function Jigsaw() {
  const { t, call, errorMessage } = usePlugin()
  const task = useTask<GameSetup>()
  const [game, setGame] = usePluginState<GameData | null>('game', null)
  const [timer, setTimer] = usePluginState<{ gameKey: string; ms: number } | null>('timer', null)
  const [saves, setSaves] = usePluginState<Record<string, SavedGame>>('saves', {})
  const [records, setRecords] = usePluginState<RecordEntry[]>('records', [])

  const [photo, setPhoto] = useState<Prepared & { name: string } | null>(null)
  const [crop, setCrop] = useState<Crop>({ x: 0, y: 0, w: 0, h: 0 })
  const [grid, setGrid] = useState({ cols: 4, rows: 4 })
  const [finished, setFinished] = useState<RecordEntry | null>(null)
  const [filter, setFilter] = useState<PieceFilter>('all')
  const [ghost, setGhost] = useState(false)
  const [camera, setCamera] = useState<Camera>({ z: 0.5, cx: 0, cy: 0 })
  const [size, setSize] = useState({ w: 0, h: 0 })
  const [gameImg, setGameImg] = useState<HTMLImageElement | null>(null)
  const [photoImg, setPhotoImg] = useState<HTMLImageElement | null>(null)
  const [hintAt, setHintAt] = useState<{ row: number; col: number; until: number } | null>(null)

  const canvasRef = useRef<HTMLCanvasElement | null>(null)
  const cropCanvasRef = useRef<HTMLCanvasElement | null>(null)
  const shellRef = useRef<HTMLDivElement | null>(null)
  const dragRef = useRef<DragState | null>(null)
  const panRef = useRef<{ sx: number; sy: number; cx: number; cy: number } | null>(null)
  const pendingStart = useRef<{ photoName: string; photoKey: string; crop: Crop; cols: number; rows: number } | null>(null)
  const fittedKey = useRef<string | null>(null)
  const spriteCache = useRef<Map<string, HTMLCanvasElement>>(new Map())

  const setup = game?.setup ?? null
  const total = setup ? setup.cols * setup.rows : 0
  const placedCount = game ? game.pieces.filter((p) => p.placed).length : 0
  const elapsed = timer && game && timer.gameKey === game.gameKey ? timer.ms : 0

  const updateGame = useCallback(
    (next: GameData) => {
      setGame(next)
    },
    [setGame],
  )

  /** 归档中断对局（LRU 上限），开新局 / 完成时调用 */
  const archiveCurrent = useCallback(
    (ms: number) => {
      if (!game) return
      setSaves((prev) => {
        const next = {
          ...prev,
          [game.gameKey]: { game, ms, savedAt: Date.now() },
        }
        const entries = Object.entries(next).sort((a, b) => b[1].savedAt - a[1].savedAt)
        return Object.fromEntries(entries.slice(0, MAX_SAVES))
      })
    },
    [game, setSaves],
  )

  const pickPhoto = async () => {
    try {
      const path = await host.dialog.openFile([{ name: t('file.filter'), extensions: IMAGE_EXTENSIONS }])
      if (!path) return
      const out = await call<Prepared>('prepare', { path })
      const name = path.split(/[\\/]/).pop() ?? path
      setPhoto({ ...out, name })
      setCrop({
        x: Math.round(out.width * 0.1),
        y: Math.round(out.height * 0.1),
        w: Math.round(out.width * 0.8),
        h: Math.round(out.height * 0.8),
      })
      setFinished(null)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const startGame = () => {
    if (!photo) return
    pendingStart.current = {
      photoName: photo.name,
      photoKey: photo.photoKey,
      crop: { ...crop },
      cols: grid.cols,
      rows: grid.rows,
    }
    task.start('newgame', {
      workData: photo.workData,
      cropX: Math.round(crop.x),
      cropY: Math.round(crop.y),
      cropW: Math.round(crop.w),
      cropH: Math.round(crop.h),
      cols: grid.cols,
      rows: grid.rows,
      seed: 0,
    })
  }

  // 开局任务完成 → 归档当前对局并进入新局
  useEffect(() => {
    if (task.status !== 'succeeded' || !task.result || !pendingStart.current) return
    const meta = pendingStart.current
    pendingStart.current = null
    const setupOut = task.result
    const gameKey = `${meta.photoKey}:${meta.crop.x},${meta.crop.y},${meta.crop.w},${meta.crop.h}:${meta.cols}x${meta.rows}`
    archiveCurrent(elapsed)
    const pieces: PieceState[] = []
    for (let row = 0; row < meta.rows; row++) {
      for (let col = 0; col < meta.cols; col++) {
        const scatter = setupOut.tray[row * meta.cols + col] ?? [0, 0]
        pieces.push({
          row,
          col,
          x: scatter[0] * setupOut.width,
          y: scatter[1] * setupOut.height,
          placed: false,
        })
      }
    }
    setFinished(null)
    setFilter('all')
    spriteCache.current.clear()
    fittedKey.current = null
    updateGame({
      gameKey,
      setup: setupOut,
      pieces,
      hints: 0,
      photoKey: meta.photoKey,
      photoName: meta.photoName,
      crop: meta.crop,
    })
    setTimer({ gameKey, ms: 0 })
  }, [task.result, task.status, archiveCurrent, elapsed, setTimer, updateGame])

  const resumeSave = (key: string) => {
    const save = saves[key]
    if (!save) return
    setSaves((prev) => {
      const next = { ...prev }
      delete next[key]
      return next
    })
    setFinished(null)
    spriteCache.current.clear()
    fittedKey.current = null
    updateGame(save.game)
    setTimer({ gameKey: key, ms: save.ms })
  }

  const discardSave = (key: string) => {
    setSaves((prev) => {
      const next = { ...prev }
      delete next[key]
      return next
    })
  }

  /** 放弃当前对局：删除存档并回到新对局面板 */
  const abandonGame = () => {
    if (!game) return
    setSaves((prev) => {
      const next = { ...prev }
      delete next[game.gameKey]
      return next
    })
    setGame(null)
    setTimer(null)
  }

  /** 重新打散未放置的拼片（已放置的保持不动） */
  const reshuffle = () => {
    if (!game || !setup) return
    const pieces = game.pieces.map((p) => {
      if (p.placed) return p
      const side = Math.floor(Math.random() * 4)
      const along = Math.random() * 1.2 - 0.1
      const band = 0.05 + Math.random() * 0.3
      const [nx, ny] =
        side === 0 ? [along, -band] : side === 1 ? [along, 1 + band] : side === 2 ? [-band, along] : [1 + band, along]
      return { ...p, x: nx * setup.width, y: ny * setup.height }
    })
    updateGame({ ...game, pieces })
  }

  /** 提示一片：放到正确位置并计数 */
  const applyHint = () => {
    if (!game || !setup) return
    const candidates = game.pieces.filter((p) => !p.placed)
    if (candidates.length === 0) return
    const pick = candidates[Math.floor(Math.random() * candidates.length)]
    const pieces = game.pieces.map((p) =>
      p === pick ? { ...p, x: p.col * setup.cellW, y: p.row * setup.cellH, placed: true } : p,
    )
    updateGame({ ...game, pieces, hints: game.hints + 1 })
    setHintAt({ row: pick.row, col: pick.col, until: Date.now() + 1500 })
    setTimeout(() => setHintAt(null), 1600)
  }

  // 计时：仅对局进行中、页面可见时累计；切走或退出即暂停
  useEffect(() => {
    if (!game || finished) return
    const id = setInterval(() => {
      if (document.visibilityState !== 'visible') return
      setTimer((prev) =>
        prev && prev.gameKey === game.gameKey ? { ...prev, ms: prev.ms + 1000 } : { gameKey: game.gameKey, ms: 1000 },
      )
    }, 1000)
    return () => clearInterval(id)
  }, [game, finished, setTimer])

  // 游戏图加载
  useEffect(() => {
    if (!setup) {
      const timer0 = setTimeout(() => setGameImg(null), 0)
      return () => clearTimeout(timer0)
    }
    const timer1 = setTimeout(() => {
      const image = new Image()
      image.onload = () => setGameImg(image)
      image.src = setup.gameData
    }, 0)
    return () => clearTimeout(timer1)
  }, [setup])

  // 取景预览图加载
  useEffect(() => {
    if (!photo) {
      const timer0 = setTimeout(() => setPhotoImg(null), 0)
      return () => clearTimeout(timer0)
    }
    const timer1 = setTimeout(() => {
      const image = new Image()
      image.onload = () => setPhotoImg(image)
      image.src = photo.workData
    }, 0)
    return () => clearTimeout(timer1)
  }, [photo])

  // 画布尺寸跟随容器
  useEffect(() => {
    const shell = shellRef.current
    if (!shell) return
    const observer = new ResizeObserver(() => {
      const rect = shell.getBoundingClientRect()
      setSize({ w: rect.width, h: rect.height })
    })
    observer.observe(shell)
    return () => observer.disconnect()
  }, [])

  // 换局时适配视野（框住画板与外围托盘散片区）
  useEffect(() => {
    if (!setup || !game || size.w === 0 || size.h === 0) return
    if (fittedKey.current === game.gameKey) return
    fittedKey.current = game.gameKey
    const z = clamp(Math.min(size.w / (setup.width * 1.8), size.h / (setup.height * 1.8)), ZOOM_MIN, 1.5)
    setCamera({ z, cx: setup.width / 2, cy: setup.height / 2 })
  }, [setup, game, size])

  // 拼片轮廓与包围盒（每局静态）
  const geometry = useMemo(() => {
    if (!setup) return null
    const outlines = []
    const boxes = []
    const paths = []
    for (const piece of game ? game.pieces : []) {
      const outline = pieceOutline(
        piece.row,
        piece.col,
        setup.cols,
        setup.rows,
        setup.eh,
        setup.ev,
        setup.cellW,
        setup.cellH,
      )
      outlines.push(outline)
      boxes.push(outlineBBox(outline))
      paths.push(outlineToPath(outline))
    }
    return { outlines, boxes, paths }
  }, [setup, game])

  const screenToBoard = useCallback(
    (sx: number, sy: number): { x: number; y: number } => {
      const canvas = canvasRef.current
      if (!canvas) return { x: 0, y: 0 }
      const rect = canvas.getBoundingClientRect()
      return {
        x: (sx - rect.left - rect.width / 2) / camera.z + camera.cx,
        y: (sy - rect.top - rect.height / 2) / camera.z + camera.cy,
      }
    },
    [camera],
  )

  // 滚轮缩放：以指针为锚点；React onWheel 是 passive，改用原生监听
  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const onWheel = (event: WheelEvent) => {
      event.preventDefault()
      const rect = canvas.getBoundingClientRect()
      const mx = event.clientX - rect.left
      const my = event.clientY - rect.top
      setCamera((prev) => {
        const z = clamp(prev.z * (event.deltaY < 0 ? 1.15 : 1 / 1.15), ZOOM_MIN, ZOOM_MAX)
        return {
          z,
          cx: prev.cx + (mx - rect.width / 2) * (1 / prev.z - 1 / z),
          cy: prev.cy + (my - rect.height / 2) * (1 / prev.z - 1 / z),
        }
      })
    }
    canvas.addEventListener('wheel', onWheel, { passive: false })
    return () => canvas.removeEventListener('wheel', onWheel)
  }, [])

  const finishIfNeeded = useCallback(
    async (next: PieceState[], gameData: GameData) => {
      if (!next.every((p) => p.placed)) return
      const setupOut = gameData.setup
      const result = await call<VerifyOut>('verify', {
        cols: setupOut.cols,
        rows: setupOut.rows,
        cellW: setupOut.cellW,
        cellH: setupOut.cellH,
        placements: next.map((p) => [p.row, p.col, p.x, p.y, p.placed ? 1 : 0]),
      })
      if (!result.valid) {
        toast.error(t('errors.jigsaw.inconsistent'))
        return
      }
      const entry: RecordEntry = {
        id: crypto.randomUUID(),
        name: gameData.photoName,
        grid: `${setupOut.cols}x${setupOut.rows}`,
        seconds: Math.round(elapsed / 1000),
        hints: gameData.hints,
        independent: gameData.hints === 0,
        finishedAt: Date.now(),
      }
      setRecords((prev) => [entry, ...prev].slice(0, MAX_RECORDS))
      setSaves((prev) => {
        const nextSaves = { ...prev }
        delete nextSaves[gameData.gameKey]
        return nextSaves
      })
      setFinished(entry)
      setGame(null)
      setTimer(null)
    },
    [call, elapsed, setGame, setRecords, setSaves, setTimer, t],
  )

  const onPointerDown = (event: React.PointerEvent) => {
    if (!game || !geometry) return
    const point = screenToBoard(event.clientX, event.clientY)
    // 从最上层未放置拼片开始命中（拖动中的片永远最后绘制，无需改数组顺序）
    for (let i = game.pieces.length - 1; i >= 0; i--) {
      const piece = game.pieces[i]
      if (piece.placed) continue
      const box = geometry.boxes[i]
      if (point.x >= piece.x + box.x && point.x <= piece.x + box.x + box.w && point.y >= piece.y + box.y && point.y <= piece.y + box.y + box.h) {
        dragRef.current = {
          index: i,
          offX: piece.x - point.x,
          offY: piece.y - point.y,
          x: piece.x,
          y: piece.y,
        }
        canvasRef.current?.setPointerCapture(event.pointerId)
        return
      }
    }
    // 空白处拖动 = 平移
    panRef.current = { sx: event.clientX, sy: event.clientY, cx: camera.cx, cy: camera.cy }
    canvasRef.current?.setPointerCapture(event.pointerId)
  }

  const onPointerMove = (event: React.PointerEvent) => {
    const drag = dragRef.current
    if (drag) {
      const point = screenToBoard(event.clientX, event.clientY)
      drag.x = point.x + drag.offX
      drag.y = point.y + drag.offY
      return
    }
    const pan = panRef.current
    if (pan) {
      setCamera((prev) => ({
        ...prev,
        cx: pan.cx - (event.clientX - pan.sx) / prev.z,
        cy: pan.cy - (event.clientY - pan.sy) / prev.z,
      }))
    }
  }

  const onPointerUp = async () => {
    panRef.current = null
    const drag = dragRef.current
    dragRef.current = null
    if (!drag || !game || !geometry) return
    const setupOut = game.setup
    const piece = game.pieces[drag.index]
    const out = await call<PlaceOut>('place', {
      row: piece.row,
      col: piece.col,
      x: drag.x,
      y: drag.y,
      cellW: setupOut.cellW,
      cellH: setupOut.cellH,
    })
    const pieces = [...game.pieces]
    if (out.placed) {
      pieces[drag.index] = { ...piece, x: out.x, y: out.y, placed: true }
    } else {
      pieces[drag.index] = { ...piece, x: drag.x, y: drag.y }
    }
    const nextGame = { ...game, pieces }
    updateGame(nextGame)
    if (out.placed) {
      try {
        await finishIfNeeded(pieces, nextGame)
      } catch (error) {
        toast.error(errorMessage(error))
      }
    }
  }

  /** 绘制一帧 */
  const drawFrame = useCallback(() => {
    const canvas = canvasRef.current
    if (!canvas || size.w === 0) return
    const dpr = window.devicePixelRatio || 1
    if (canvas.width !== Math.round(size.w * dpr) || canvas.height !== Math.round(size.h * dpr)) {
      canvas.width = Math.round(size.w * dpr)
      canvas.height = Math.round(size.h * dpr)
    }
    const ctx = canvas.getContext('2d')
    if (!ctx) return
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    ctx.clearRect(0, 0, size.w, size.h)
    if (!setup || !geometry || !game) return
    const { z, cx, cy } = camera
    const toSx = (bx: number) => (bx - cx) * z + size.w / 2
    const toSy = (by: number) => (by - cy) * z + size.h / 2

    // 画板
    ctx.fillStyle = 'rgba(255,255,255,0.92)'
    ctx.fillRect(toSx(0), toSy(0), setup.width * z, setup.height * z)
    ctx.strokeStyle = 'rgba(15,23,42,0.25)'
    ctx.lineWidth = 1
    ctx.strokeRect(toSx(0), toSy(0), setup.width * z, setup.height * z)
    // 格位参考线
    ctx.strokeStyle = 'rgba(15,23,42,0.08)'
    for (let col = 1; col < setup.cols; col++) {
      const x = toSx(col * setup.cellW)
      ctx.beginPath()
      ctx.moveTo(x, toSy(0))
      ctx.lineTo(x, toSy(setup.height))
      ctx.stroke()
    }
    for (let row = 1; row < setup.rows; row++) {
      const y = toSy(row * setup.cellH)
      ctx.beginPath()
      ctx.moveTo(toSx(0), y)
      ctx.lineTo(toSx(setup.width), y)
      ctx.stroke()
    }

    // 淡原图提示
    if (ghost && gameImg) {
      ctx.save()
      ctx.globalAlpha = 0.15
      ctx.drawImage(gameImg, toSx(0), toSy(0), setup.width * z, setup.height * z)
      ctx.restore()
    }

    const sprite = (index: number, alpha: number, at?: { x: number; y: number }) => {
      const piece = game.pieces[index]
      const pos = at ?? { x: piece.x, y: piece.y }
      const box = geometry.boxes[index]
      const key = `${index}:${Math.round(z * 100)}`
      let spriteCanvas = spriteCache.current.get(key)
      if (!spriteCanvas && gameImg) {
        spriteCanvas = document.createElement('canvas')
        const s = z * dpr
        spriteCanvas.width = Math.max(1, Math.ceil(box.w * s))
        spriteCanvas.height = Math.max(1, Math.ceil(box.h * s))
        const sctx = spriteCanvas.getContext('2d')
        if (sctx) {
          sctx.scale(s, s)
          sctx.translate(-box.x, -box.y)
          sctx.clip(geometry.paths[index])
          sctx.drawImage(gameImg, 0, 0, gameImg.naturalWidth, gameImg.naturalHeight, 0, 0, setup.width, setup.height)
          sctx.lineWidth = 1 / z
          sctx.strokeStyle = 'rgba(15,23,42,0.45)'
          sctx.stroke(geometry.paths[index])
        }
        if (spriteCache.current.size > 160) spriteCache.current.clear()
        spriteCache.current.set(key, spriteCanvas)
      }
      if (!spriteCanvas) return
      ctx.save()
      ctx.globalAlpha = alpha
      ctx.drawImage(spriteCanvas, toSx(pos.x + box.x), toSy(pos.y + box.y), box.w * z, box.h * z)
      ctx.restore()
    }

    // 已放置的在底层，散片在上层
    game.pieces.forEach((piece, index) => {
      if (piece.placed) sprite(index, 1)
    })
    const drag = dragRef.current
    game.pieces.forEach((piece, index) => {
      if (piece.placed || index === drag?.index) return
      const category = pieceCategory(piece.row, piece.col, setup.cols, setup.rows)
      const dim = filter !== 'all' && category !== filter
      sprite(index, dim ? 0.22 : 1)
    })

    // 拖动中的拼片 + 目标提示
    if (drag) {
      const piece = game.pieces[drag.index]
      const targetX = piece.col * setup.cellW
      const targetY = piece.row * setup.cellH
      const tolerance = DRAG_TOLERANCE_RATIO * Math.min(setup.cellW, setup.cellH)
      const dx = drag.x - targetX
      const dy = drag.y - targetY
      const near = dx * dx + dy * dy <= tolerance * tolerance
      ctx.strokeStyle = near ? 'rgba(34,197,94,0.9)' : 'rgba(59,130,246,0.5)'
      ctx.lineWidth = 1.5
      ctx.setLineDash([4, 3])
      ctx.strokeRect(
        toSx(targetX),
        toSy(targetY),
        setup.cellW * z,
        setup.cellH * z,
      )
      ctx.setLineDash([])
      sprite(drag.index, 0.95, { x: drag.x, y: drag.y })
    }

    // 提示高亮
    if (hintAt && Date.now() < hintAt.until) {
      ctx.strokeStyle = 'rgba(234,179,8,0.95)'
      ctx.lineWidth = 2.5
      ctx.strokeRect(
        toSx(hintAt.col * setup.cellW),
        toSy(hintAt.row * setup.cellH),
        setup.cellW * z,
        setup.cellH * z,
      )
    }
  }, [camera, filter, game, geometry, ghost, gameImg, hintAt, setup, size])

  useEffect(() => {
    let raf = 0
    const loop = () => {
      drawFrame()
      raf = requestAnimationFrame(loop)
    }
    raf = requestAnimationFrame(loop)
    return () => cancelAnimationFrame(raf)
  }, [drawFrame])

  // 取景预览容器宽度跟随面板
  const [cropWidth, setCropWidth] = useState(0)
  useEffect(() => {
    const shell = cropCanvasRef.current?.parentElement
    if (!shell) return
    const observer = new ResizeObserver(() => setCropWidth(shell.clientWidth))
    observer.observe(shell)
    return () => observer.disconnect()
  }, [photo])

  /** 取景画布：拖拽划出裁剪范围 */
  const drawCrop = useCallback(() => {
    const canvas = cropCanvasRef.current
    if (!canvas || !photoImg || !photo) return
    const dpr = window.devicePixelRatio || 1
    const maxW = cropWidth || canvas.parentElement?.clientWidth || photo.width
    const scale = Math.min(1, maxW / photo.width)
    const dispW = Math.round(photo.width * scale)
    const dispH = Math.round(photo.height * scale)
    if (canvas.width !== Math.round(dispW * dpr)) {
      canvas.width = Math.round(dispW * dpr)
      canvas.height = Math.round(dispH * dpr)
      canvas.style.width = `${dispW}px`
      canvas.style.height = `${dispH}px`
    }
    const ctx = canvas.getContext('2d')
    if (!ctx) return
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    ctx.clearRect(0, 0, dispW, dispH)
    ctx.drawImage(photoImg, 0, 0, dispW, dispH)
    ctx.fillStyle = 'rgba(15,23,42,0.55)'
    ctx.fillRect(0, 0, dispW, dispH)
    ctx.drawImage(
      photoImg,
      crop.x * scale,
      crop.y * scale,
      crop.w * scale,
      crop.h * scale,
      crop.x * scale,
      crop.y * scale,
      crop.w * scale,
      crop.h * scale,
    )
    ctx.strokeStyle = 'rgba(56,189,248,0.95)'
    ctx.lineWidth = 1.5
    ctx.strokeRect(crop.x * scale, crop.y * scale, crop.w * scale, crop.h * scale)
  }, [crop, cropWidth, photo, photoImg])

  useEffect(() => {
    const timer0 = setTimeout(() => drawCrop(), 0)
    return () => clearTimeout(timer0)
  }, [drawCrop])

  const cropPointer = (event: React.PointerEvent): { x: number; y: number } => {
    const canvas = cropCanvasRef.current
    if (!canvas || !photo) return { x: 0, y: 0 }
    const rect = canvas.getBoundingClientRect()
    const scale = photo.width / rect.width
    return {
      x: clamp((event.clientX - rect.left) * scale, 0, photo.width),
      y: clamp((event.clientY - rect.top) * scale, 0, photo.height),
    }
  }

  const cropDrag = useRef<{ sx: number; sy: number } | null>(null)

  const onCropDown = (event: React.PointerEvent) => {
    const point = cropPointer(event)
    cropDrag.current = { sx: point.x, sy: point.y }
    setCrop({ x: point.x, y: point.y, w: 1, h: 1 })
    cropCanvasRef.current?.setPointerCapture(event.pointerId)
  }

  const onCropMove = (event: React.PointerEvent) => {
    const drag = cropDrag.current
    if (!drag || !photo) return
    const point = cropPointer(event)
    setCrop({
      x: Math.min(drag.sx, point.x),
      y: Math.min(drag.sy, point.y),
      w: Math.abs(point.x - drag.sx),
      h: Math.abs(point.y - drag.sy),
    })
  }

  const onCropUp = () => {
    cropDrag.current = null
    setCrop((prev) => {
      const w = Math.max(prev.w, MIN_CROP)
      const h = Math.max(prev.h, MIN_CROP)
      const x = clamp(prev.x, 0, (photo?.width ?? w) - w)
      const y = clamp(prev.y, 0, (photo?.height ?? h) - h)
      return { x, y, w, h }
    })
  }

  const cropValid = crop.w >= MIN_CROP && crop.h >= MIN_CROP
  const saveEntries = Object.entries(saves).sort((a, b) => b[1].savedAt - a[1].savedAt)
  const progress = task.progress && task.progress.total > 0 ? (task.progress.done / task.progress.total) * 100 : null

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(300px,0.8fr)_minmax(0,1.4fr)] gap-3">
      <div className="flex min-h-0 flex-col gap-3">
        {game ? (
          <Panel icon={<Puzzle />} title={t('game.title')} className="shrink-0" bodyClassName="space-y-3 p-3">
            <div className="flex flex-wrap items-center gap-2 text-xs text-fg-muted tabular-nums">
              <Badge variant="info">{t('game.progress', { placed: placedCount, total })}</Badge>
              <Badge>{t('game.clock', { time: formatClock(elapsed) })}</Badge>
              <Badge>{t('game.hints', { count: game.hints })}</Badge>
              <Badge>{t('file.name', { name: game.photoName })}</Badge>
            </div>
            <div className="flex flex-wrap items-center gap-1.5">
              {(['all', 'edge', 'corner', 'inner'] as const).map((mode) => (
                <Button
                  key={mode}
                  size="sm"
                  variant={filter === mode ? 'primary' : 'outline'}
                  onClick={() => setFilter(mode)}
                >
                  {t(`filter.${mode}`)}
                </Button>
              ))}
            </div>
            <div className="grid grid-cols-2 gap-2">
              <Button variant="outline" onClick={applyHint}>
                <Lightbulb />
                {t('game.hint')}
              </Button>
              <Button variant="outline" onClick={reshuffle}>
                <Shuffle />
                {t('game.reshuffle')}
              </Button>
              <Button variant="outline" onClick={() => setGhost((prev) => !prev)}>
                {ghost ? <EyeOff /> : <Eye />}
                {t('game.ghost')}
              </Button>
              <Button variant="outline" onClick={abandonGame}>
                <Trash2 />
                {t('game.abandon')}
              </Button>
            </div>
            <p className="text-[11px] text-fg-subtle">{t('game.hintText')}</p>
          </Panel>
        ) : (
          <Panel
            icon={<ImagePlus />}
            title={t('setup.title')}
            className="min-h-0 flex-1"
            bodyClassName="space-y-3.5 overflow-auto p-3"
          >
            <Button block variant="outline" onClick={pickPhoto} disabled={task.running}>
              <ImagePlus />
              {t('setup.pick')}
            </Button>
            {photo && (
              <>
                <div className="space-y-1.5">
                  <p className="text-xs font-medium text-fg-muted">{t('setup.crop')}</p>
                  <div className="overflow-hidden rounded-card border border-border">
                    <canvas
                      ref={cropCanvasRef}
                      onPointerDown={onCropDown}
                      onPointerMove={onCropMove}
                      onPointerUp={onCropUp}
                      className="block w-full cursor-crosshair touch-none"
                      aria-label={t('setup.crop')}
                    />
                  </div>
                  <p className="text-[11px] text-fg-subtle">
                    {t('setup.cropHint', { width: Math.round(crop.w), height: Math.round(crop.h) })}
                  </p>
                </div>
                <div className="space-y-1.5">
                  <p className="text-xs font-medium text-fg-muted">{t('setup.grid')}</p>
                  <SegmentedControl<string>
                    value={String(grid.cols)}
                    onValueChange={(value) => {
                      const cols = Number(value)
                      setGrid({ cols, rows: cols })
                    }}
                    aria-label={t('setup.grid')}
                    options={GRIDS.map((g) => ({
                      value: String(g.cols),
                      label: t('setup.gridItem', { cols: g.cols, rows: g.rows, count: g.cols * g.rows }),
                    }))}
                  />
                </div>
                {task.running ? (
                  <div className="space-y-1.5">
                    <div className="h-1.5 overflow-hidden rounded-full bg-active">
                      <div className="h-full rounded-full bg-primary transition-all" style={{ width: `${progress ?? 0}%` }} />
                    </div>
                    <Button block onClick={task.cancel}>
                      {t('setup.cancelGenerate')}
                    </Button>
                  </div>
                ) : (
                  <Button block variant="primary" disabled={!cropValid} onClick={startGame}>
                    <Play />
                    {t('setup.start')}
                  </Button>
                )}
                {task.status === 'failed' && task.error && (
                  <p className="text-xs text-danger">{errorMessage(task.error)}</p>
                )}
              </>
            )}
            {!photo && <p className="text-xs text-fg-subtle">{t('setup.hint')}</p>}
          </Panel>
        )}

        {saveEntries.length > 0 && (
          <Panel icon={<Bookmark />} title={t('saves.title')} className="shrink-0" bodyClassName="p-3">
            <ul className="space-y-1">
              {saveEntries.map(([key, save]) => (
                <li key={key} className="flex items-center gap-1">
                  <button
                    type="button"
                    onClick={() => resumeSave(key)}
                    className="min-w-0 flex-1 truncate rounded-control px-2 py-1 text-left text-xs hover:bg-hover"
                  >
                    {t('saves.item', {
                      name: save.game.photoName,
                      cols: save.game.setup.cols,
                      rows: save.game.setup.rows,
                      placed: save.game.pieces.filter((p) => p.placed).length,
                      total: save.game.setup.cols * save.game.setup.rows,
                    })}
                  </button>
                  <Tooltip content={t('saves.remove')}>
                    <Button size="icon-sm" variant="ghost" aria-label={t('saves.remove')} onClick={() => discardSave(key)}>
                      <Trash2 />
                    </Button>
                  </Tooltip>
                </li>
              ))}
            </ul>
          </Panel>
        )}

        {records.length > 0 && (
          <Panel
            icon={<Trophy />}
            title={t('records.title')}
            className="min-h-0 flex-1"
            bodyClassName="p-3"
            actions={
              <Button size="sm" variant="ghost" onClick={() => setRecords([])}>
                <Trash2 />
                {t('records.clear')}
              </Button>
            }
          >
            <ul className="max-h-full space-y-1 overflow-auto">
              {records.map((entry) => (
                <li key={entry.id} className="flex items-center gap-2 rounded-control px-2 py-1 text-xs hover:bg-hover">
                  <span className="min-w-0 flex-1 truncate">{entry.name}</span>
                  <Badge>{entry.grid}</Badge>
                  <span className="tabular-nums text-fg-muted">{formatClock(entry.seconds * 1000)}</span>
                  <Badge variant={entry.independent ? 'success' : 'info'}>
                    {t(entry.independent ? 'records.independent' : 'records.withHints', { count: entry.hints })}
                  </Badge>
                </li>
              ))}
            </ul>
          </Panel>
        )}
      </div>

      <Panel
        icon={<Puzzle />}
        title={t('canvas.title')}
        className="min-h-0"
        bodyClassName="relative p-0"
        footer={
          <span className="text-xs text-fg-subtle">{t('canvas.hint')}</span>
        }
      >
        <div ref={shellRef} className="absolute inset-0 bg-surface-2">
          {game && geometry ? (
            <canvas
              ref={canvasRef}
              onPointerDown={onPointerDown}
              onPointerMove={onPointerMove}
              onPointerUp={onPointerUp}
              onPointerLeave={onPointerUp}
              className="size-full touch-none"
              aria-label={t('canvas.title')}
            />
          ) : (
            <div className="flex size-full items-center justify-center">
              {task.running ? (
                <Spinner className="size-6 text-fg-subtle" />
              ) : (
                <Empty icon={<Puzzle />} title={t('canvas.empty')} description={t('canvas.emptyHint')} />
              )}
            </div>
          )}
          {finished && (
            <div className="absolute inset-0 flex items-center justify-center bg-surface-2/80 p-6">
              <div className="w-full max-w-sm space-y-3 rounded-card border border-border bg-surface p-5 text-center shadow-card">
                <Trophy className="mx-auto size-8 text-amber-500" />
                <p className="text-sm font-medium">{t('finish.title')}</p>
                <div className="flex flex-wrap items-center justify-center gap-2 text-xs text-fg-muted tabular-nums">
                  <Badge variant="info">{t('game.clock', { time: formatClock(finished.seconds * 1000) })}</Badge>
                  <Badge>{t('game.hints', { count: finished.hints })}</Badge>
                  <Badge variant={finished.independent ? 'success' : 'info'}>
                    {t(finished.independent ? 'records.independent' : 'records.withHints', { count: finished.hints })}
                  </Badge>
                </div>
                <p className="text-xs text-fg-subtle">{t('finish.saved')}</p>
                <Button block variant="primary" onClick={() => setFinished(null)}>
                  {t('finish.done')}
                </Button>
              </div>
            </div>
          )}
        </div>
      </Panel>
    </div>
  )
}
