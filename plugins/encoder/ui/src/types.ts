export type Codec = 'base64' | 'base64url' | 'base32' | 'hex' | 'urlComponent' | 'url' | 'form' | 'html' | 'unicode'
export type Direction = 'encode' | 'decode'
export type HtmlMode = 'basic' | 'decimal' | 'hex'
export type UnicodeStyle = 'u' | 'braces' | 'codepoint'
export type FileFormat = 'base64' | 'dataUri'

export const GROUPS: { key: string; codecs: Codec[] }[] = [
  { key: 'binary', codecs: ['base64', 'base64url', 'base32', 'hex'] },
  { key: 'url', codecs: ['urlComponent', 'url', 'form'] },
  { key: 'text', codecs: ['html', 'unicode'] },
]

export interface Options {
  padding: boolean
  wrap: boolean
  uppercase: boolean
  htmlMode: HtmlMode
  unicodeStyle: UnicodeStyle
  escapeAscii: boolean
}

export interface TransformResult {
  output: string
  text: boolean
  inputBytes: number
  outputBytes: number
  elapsedMs: number
}

export interface FileResult {
  output: string
  name: string
  mime: string
  bytes: number
}
