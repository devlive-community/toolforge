export type Codec = 'base64' | 'base64url' | 'base32' | 'hex' | 'base58' | 'base85' | 'ascii85' | 'urlComponent' | 'url' | 'form' | 'html' | 'unicode' | 'morse'
export type Direction = 'encode' | 'decode'
export type HtmlMode = 'basic' | 'decimal' | 'hex'
export type UnicodeStyle = 'u' | 'braces' | 'codepoint'
export type FileFormat = 'base64' | 'dataUri'

export const GROUPS: { key: string; codecs: Codec[] }[] = [
  { key: 'binary', codecs: ['base64', 'base64url', 'base32', 'hex', 'base58', 'base85', 'ascii85'] },
  { key: 'url', codecs: ['urlComponent', 'url', 'form'] },
  { key: 'text', codecs: ['html', 'unicode', 'morse'] },
]

export interface Options {
  padding: boolean
  wrap: boolean
  uppercase: boolean
  htmlMode: HtmlMode
  unicodeStyle: UnicodeStyle
  escapeAscii: boolean
  morseUtf8: boolean
}

export interface TransformResult {
  output: string
  text: boolean
  inputBytes: number
  outputBytes: number
  elapsedMs: number
}

export interface FileResult {
  /** 后端缓存完整结果的编号，用于复制 / 保存 */
  id: number
  /** 仅前若干字符；完整结果留在后端，避免渲染超大文本卡死界面 */
  preview: string
  truncated: boolean
  chars: number
  name: string
  mime: string
  bytes: number
}
