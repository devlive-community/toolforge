/** prepare 的输出：受限工作图（PNG Data URI）与像素尺寸 */
export interface PrepareResult {
  width: number
  height: number
  workData: string
}

/** convert 的输出：结果 id + 有界预览与统计 */
export interface ConvertResult {
  id: string
  text: string
  complete: boolean
  cols: number
  rows: number
  chars: number
  warnings: string[]
}

/** fetch / save 的输出 */
export interface FetchResult {
  text: string
}

/** 可持久化的参数方案（不含图片） */
export interface ArtParams {
  width: number
  preset: string
  custom: string
  brightness: number
  contrast: number
  invert: boolean
  color: boolean
  aspect: number
  alphaMode: 'keep' | 'fill'
  bg: string
}

export const WIDTH_MIN = 16
export const WIDTH_MAX = 320
export const ASPECT_MIN = 0.3
export const ASPECT_MAX = 1

export const DEFAULT_PARAMS: ArtParams = {
  width: 80,
  preset: 'classic',
  custom: '',
  brightness: 0,
  contrast: 1,
  invert: false,
  color: false,
  aspect: 0.5,
  alphaMode: 'keep',
  bg: 'ffffff',
}

export const PRESET_IDS = ['classic', 'blocks', 'minimal', 'dots', 'binary'] as const

export const IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp', 'bmp', 'gif', 'tif', 'tiff']
