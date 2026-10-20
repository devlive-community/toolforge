export interface Mesh {
  points: [number, number][]
  triangles: [number, number, number][]
  colors: string[]
}

export interface Project {
  /** 受限工作图（PNG Data URI），原图移动后仍可继续 */
  workData: string
  width: number
  height: number
  seed: number
  count: number
  detail: number
  points: [number, number][]
  brightness: number
  strokeWidth: number
  strokeColor: string
  background: string
  scale: number
  /** 自动生成的初始点集标记；手动编辑后为 false */
  auto: boolean
}

export const DEFAULT_PROJECT: Omit<Project, 'workData' | 'width' | 'height'> = {
  seed: 1,
  count: 250,
  detail: 60,
  points: [],
  brightness: 0,
  strokeWidth: 0,
  strokeColor: '111111',
  background: 'ffffff',
  scale: 1,
  auto: true,
}

export type ViewMode = 'edit' | 'result' | 'original'

export const MAX_POINTS = 2000
export const MAX_SCALE = 3
