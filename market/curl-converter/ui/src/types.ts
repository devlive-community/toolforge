import type { AppError } from '@toolforge/plugin-ui-sdk'

export type Language = 'javascript' | 'python' | 'go' | 'rust' | 'java' | 'php' | 'csharp'

export interface Part {
  name: string
  value: string | null
  file: string | null
  filename: string | null
  contentType: string | null
}

export type Body =
  | { kind: 'none' }
  | { kind: 'text'; text: string }
  | { kind: 'file'; path: string }
  | { kind: 'multipart'; parts: Part[] }

export interface Request {
  method: string
  url: string
  headers: [string, string][]
  body: Body
  insecure: boolean
  followRedirects: boolean
  compressed: boolean
  timeout: number | null
  proxy: string | null
}

export interface Converted {
  code: string
  request: Request
  warnings: AppError[]
}
