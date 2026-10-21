/** 拼图游戏类型与拼片轮廓几何 */

export interface Crop {
  x: number
  y: number
  w: number
  h: number
}

/** 一条内部边的形状参数：[sign, m, h]；sign=+1 凸片属于左/上片 */
export type EdgeSpec = [number, number, number]

export interface GameSetup {
  gameData: string
  width: number
  height: number
  cols: number
  rows: number
  seed: number
  cellW: number
  cellH: number
  eh: EdgeSpec[][]
  ev: EdgeSpec[][]
  /** 未放置拼片初始散落位置（相对画板宽高，可为负） */
  tray: [number, number][]
}

export interface PieceState {
  row: number
  col: number
  /** 拼片格位起点（画板坐标） */
  x: number
  y: number
  placed: boolean
}

export interface GameData {
  gameKey: string
  setup: GameSetup
  pieces: PieceState[]
  hints: number
  photoKey: string
  photoName: string
  crop: Crop
}

export interface RecordEntry {
  id: string
  name: string
  grid: string
  seconds: number
  hints: number
  independent: boolean
  finishedAt: number
}

export interface Prepared {
  workData: string
  width: number
  height: number
  photoKey: string
}

export interface PlaceOut {
  placed: boolean
  x: number
  y: number
}

export interface VerifyOut {
  placed: number
  total: number
  valid: boolean
}

export const GRIDS: Array<{ cols: number; rows: number }> = [
  { cols: 3, rows: 3 },
  { cols: 4, rows: 4 },
  { cols: 6, rows: 6 },
]

export const MAX_SAVES = 3
export const MAX_RECORDS = 50

/** 拼片类别：filter 用 */
export type PieceFilter = 'all' | 'edge' | 'corner' | 'inner'

export function pieceCategory(row: number, col: number, cols: number, rows: number): PieceFilter {
  const onTopBottom = row === 0 || row === rows - 1
  const onLeftRight = col === 0 || col === cols - 1
  if (onTopBottom && onLeftRight) return 'corner'
  if (onTopBottom || onLeftRight) return 'edge'
  return 'inner'
}

interface Pt {
  x: number
  y: number
}

interface Side {
  start: Pt
  segs: Array<{ c1: Pt; c2: Pt; p: Pt }>
}

function straightSide(from: Pt, to: Pt): Side {
  const lerp = (a: number, b: number, t: number) => a + (b - a) * t
  return {
    start: from,
    segs: [
      {
        c1: { x: lerp(from.x, to.x, 1 / 3), y: lerp(from.y, to.y, 1 / 3) },
        c2: { x: lerp(from.x, to.x, 2 / 3), y: lerp(from.y, to.y, 2 / 3) },
        p: to,
      },
    ],
  }
}

/** 反向遍历一条边（控制点随之调换），保证与相邻片逐点重合 */
function reverseSide(side: Side): Side {
  const segs: Side['segs'] = []
  for (let i = side.segs.length - 1; i >= 0; i--) {
    const seg = side.segs[i]
    const start = i === 0 ? side.start : side.segs[i - 1].p
    segs.push({ c1: seg.c2, c2: seg.c1, p: start })
  }
  return { start: side.segs[side.segs.length - 1].p, segs }
}

/**
 * 凹凸边形状（单位 = 凸起高度）：t 沿边 0..1，u 垂直凸出 0..1。
 * 脖颈比球部窄，形成经典拼图凹凸扣。
 */
function tabSegments(m: number): Array<[number, number, number, number, number, number]> {
  const a = m - 0.15
  const b = m + 0.15
  const n1 = m - 0.08
  const n2 = m + 0.08
  return [
    [a / 3, 0, (2 * a) / 3, 0, a, 0],
    [m - 0.115, 0, n1, 0.1, n1, 0.3],
    [n1, 0.62, m - 0.17, 0.74, m - 0.13, 0.88],
    [m - 0.1, 1.0, m - 0.05, 1.0, m, 1.0],
    [m + 0.05, 1.0, m + 0.1, 1.0, m + 0.13, 0.88],
    [m + 0.17, 0.74, n2, 0.62, n2, 0.3],
    [n2, 0.1, m + 0.115, 0, b, 0],
    [b + (1 - b) / 3, 0, b + (2 * (1 - b)) / 3, 0, 1, 0],
  ]
}

/** 水平内边（绝对几何按 L→R 参数化）；yBase 为该边在片内的 y */
function horizontalSide(
  edge: EdgeSpec | null,
  yBase: number,
  cw: number,
  ch: number,
  forward: boolean,
): Side {
  if (!edge) {
    return straightSide(
      { x: forward ? 0 : cw, y: yBase },
      { x: forward ? cw : 0, y: yBase },
    )
  }
  const [sign, m, h] = edge
  const hh = h * ch
  const point = (t: number, u: number): Pt => ({ x: t * cw, y: yBase + sign * u * hh })
  const side: Side = {
    start: point(0, 0),
    segs: tabSegments(m).map(([c1t, c1u, c2t, c2u, pt, pu]) => ({
      c1: point(c1t, c1u),
      c2: point(c2t, c2u),
      p: point(pt, pu),
    })),
  }
  return forward ? side : reverseSide(side)
}

/** 垂直内边（绝对几何按 T→B 参数化）；xBase 为该边在片内的 x */
function verticalSide(
  edge: EdgeSpec | null,
  xBase: number,
  cw: number,
  ch: number,
  forward: boolean,
): Side {
  if (!edge) {
    return straightSide(
      { x: xBase, y: forward ? 0 : ch },
      { x: xBase, y: forward ? ch : 0 },
    )
  }
  const [sign, m, h] = edge
  const hh = h * cw
  const point = (t: number, u: number): Pt => ({ x: xBase + sign * u * hh, y: t * ch })
  const side: Side = {
    start: point(0, 0),
    segs: tabSegments(m).map(([c1t, c1u, c2t, c2u, pt, pu]) => ({
      c1: point(c1t, c1u),
      c2: point(c2t, c2u),
      p: point(pt, pu),
    })),
  }
  return forward ? side : reverseSide(side)
}

export interface PieceOutline {
  start: Pt
  segs: Array<{ c1: Pt; c2: Pt; p: Pt }>
}

/**
 * 拼片轮廓（相对格位原点，顺时针）。相邻片共享同一条绝对曲线，
 * 因此凹凸边天然互相吻合、无缝隙；边界边为直线。
 */
export function pieceOutline(
  row: number,
  col: number,
  cols: number,
  rows: number,
  eh: EdgeSpec[][],
  ev: EdgeSpec[][],
  cw: number,
  ch: number,
): PieceOutline {
  const top = horizontalSide(row === 0 ? null : eh[row - 1][col], 0, cw, ch, true)
  const right = verticalSide(col === cols - 1 ? null : ev[row][col], cw, cw, ch, true)
  const bottom = horizontalSide(row === rows - 1 ? null : eh[row][col], ch, cw, ch, false)
  const left = verticalSide(col === 0 ? null : ev[row][col - 1], 0, cw, ch, false)
  return {
    start: top.start,
    segs: [...top.segs, ...right.segs, ...bottom.segs, ...left.segs],
  }
}

/** 轮廓包围盒（含控制点，四周留 1px 余量）；可为负（凸起越过格位边） */
export function outlineBBox(outline: PieceOutline): { x: number; y: number; w: number; h: number } {
  const pts: Pt[] = [outline.start]
  for (const seg of outline.segs) {
    pts.push(seg.c1, seg.c2, seg.p)
  }
  const xs = pts.map((p) => p.x)
  const ys = pts.map((p) => p.y)
  const x = Math.min(...xs) - 1
  const y = Math.min(...ys) - 1
  return { x, y, w: Math.max(...xs) + 1 - x, h: Math.max(...ys) + 1 - y }
}

/** 把轮廓转为 Path2D（可复用于裁剪与描边） */
export function outlineToPath(outline: PieceOutline): Path2D {
  const path = new Path2D()
  path.moveTo(outline.start.x, outline.start.y)
  for (const seg of outline.segs) {
    path.bezierCurveTo(seg.c1.x, seg.c1.y, seg.c2.x, seg.c2.y, seg.p.x, seg.p.y)
  }
  path.closePath()
  return path
}

export function formatClock(ms: number): string {
  const total = Math.floor(ms / 1000)
  const minutes = String(Math.floor(total / 60)).padStart(2, '0')
  const seconds = String(total % 60).padStart(2, '0')
  return `${minutes}:${seconds}`
}
