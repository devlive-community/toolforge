export type FractalKind = 'mandelbrot' | 'julia'
export type PaletteName = 'spectrum' | 'ember' | 'ocean' | 'mono'
export type Brightness = 'normal' | 'bright' | 'dim'

/** 一次视角快照：几何 + 风格整体，撤销与收藏共用 */
export interface View {
  fractal: FractalKind
  centerX: number
  centerY: number
  /** 复平面水平跨度 */
  spanX: number
  juliaX: number
  juliaY: number
  maxIter: number
  palette: PaletteName
  brightness: Brightness
}

export interface Favorite {
  id: string
  view: View
}

export interface Project {
  view: View
  favorites: Favorite[]
}

export const PREVIEW_WIDTH = 480
export const PREVIEW_HEIGHT = 320
/** 与后端 fractal::PRECISION_LIMIT 一致 */
export const MIN_SPAN = 1e-13
export const MAX_SPAN = 8
export const MAX_ITER = 5000
export const MAX_EXPORT_PIXELS = 2_500_000

export const DEFAULT_VIEW: View = {
  fractal: 'mandelbrot',
  centerX: -0.6,
  centerY: 0,
  spanX: 3.2,
  juliaX: -0.8,
  juliaY: 0.156,
  maxIter: 256,
  palette: 'spectrum',
  brightness: 'normal',
}

export const DEFAULT_PROJECT: Project = {
  view: { ...DEFAULT_VIEW },
  favorites: [],
}

export const PALETTES: PaletteName[] = ['spectrum', 'ember', 'ocean', 'mono']

export interface PreviewOut {
  image: string
  width: number
  height: number
  maxIter: number
  interiorRatio: number
  precisionLimited: boolean
  elapsedMs: number
}

export type PreviewStats = Pick<PreviewOut, 'interiorRatio' | 'precisionLimited' | 'elapsedMs'>

export interface ExportReport {
  output: string
  width: number
  height: number
  interiorRatio: number
  precisionLimited: boolean
  elapsedMs: number
}
