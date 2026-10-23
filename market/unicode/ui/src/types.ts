export type Flag = 'invisible' | 'bidi' | 'confusable' | 'space' | 'control' | 'combining' | 'unassigned'

export interface CharInfo {
  index: number
  offset: number
  char: string
  code: string
  name: string | null
  category: string
  script: string
  utf8: string
  utf16: string
  flags: Flag[]
  lookalike: string | null
}

export interface Analysis {
  counts: {
    chars: number
    graphemes: number
    utf8Bytes: number
    utf16Units: number
    lines: number
    invisible: number
    bidi: number
    confusable: number
    space: number
    control: number
  }
  chars: CharInfo[]
  truncated: boolean
  mixedWords: string[]
  normalization: { nfc: boolean; nfd: boolean; nfkc: boolean; nfkd: boolean }
}

export interface Found {
  char: string
  code: string
  name: string | null
  category: string
  script: string
  decimal: number
  utf8: string
  utf16: string
  flags: Flag[]
  escapes: Record<'rust' | 'javascript' | 'python' | 'java' | 'html' | 'css' | 'url', string>
}

export const FLAG_TONE: Record<Flag, 'danger' | 'warning' | 'info' | 'neutral'> = {
  bidi: 'danger',
  invisible: 'danger',
  confusable: 'warning',
  control: 'warning',
  space: 'info',
  combining: 'neutral',
  unassigned: 'neutral',
}
