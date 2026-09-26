export interface Line {
  text: string
  score: number
  x: number
  y: number
  w: number
  h: number
}

export interface Output {
  name: string | null
  width: number
  height: number
  preview: string
  lines: Line[]
  text: string
  elapsedMs: number
}

export type Source = { kind: 'file'; path: string } | { kind: 'clipboard' }
