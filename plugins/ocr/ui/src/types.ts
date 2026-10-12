export interface Line {
  text: string
  score: number
  x: number
  y: number
  w: number
  h: number
}

export interface ImageOutput {
  kind: 'image'
  name: string | null
  width: number
  height: number
  preview: string
  lines: Line[]
  text: string
  elapsedMs: number
}

export interface PageText {
  page: number
  lines: Line[]
  text: string
}

export interface PdfOutput {
  kind: 'pdf'
  name: string | null
  pages: PageText[]
  text: string
  elapsedMs: number
}

export type Output = ImageOutput | PdfOutput

export type Source = { kind: 'file'; path: string } | { kind: 'pdf'; path: string } | { kind: 'clipboard' }
