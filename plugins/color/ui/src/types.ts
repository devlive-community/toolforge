export interface Contrast {
  ratio: number
  aa: boolean
  aaa: boolean
  aaLarge: boolean
  aaaLarge: boolean
}

export interface Converted {
  hex: string
  alpha: number
  name: string | null
  formats: { key: string; value: string }[]
  luminance: number
  text: string
  onWhite: Contrast
  onBlack: Contrast
  compare: Contrast | null
  compareHex: string | null
  tints: string[]
  shades: string[]
  harmonies: { key: string; colors: string[] }[]
}
