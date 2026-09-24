import type { AppError } from '@toolforge/plugin-ui-sdk'

export type ModelId = 'u2netp' | 'isnet-general-use'
export const MODELS: ModelId[] = ['u2netp', 'isnet-general-use']

export type BackgroundMode = 'transparent' | 'white' | 'custom'
export type Format = 'png' | 'jpeg'

export interface FileResult {
  path: string
  name: string
  output: string | null
  width: number
  height: number
  bytes: number
  ms: number
  preview: string | null
  transparent: boolean
  error: AppError | null
}

export interface Report {
  files: FileResult[]
  succeeded: number
  elapsedMs: number
}
