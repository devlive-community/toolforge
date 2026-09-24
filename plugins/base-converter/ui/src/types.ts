export interface Width {
  bits: number
  hex: string
  binary: string
  unsigned: string
  signed: string
}

export interface Converted {
  base: number
  source: string
  negative: boolean
  binary: string
  octal: string
  decimal: string
  hex: string
  custom: string
  customBase: number
  bitLength: number
  byteLength: number
  widths: Width[]
  bits: string | null
}

/** 输入进制：0 表示按前缀自动识别 */
export const INPUT_BASES = [0, 2, 8, 10, 16, ...Array.from({ length: 35 }, (_, i) => i + 2).filter((b) => ![2, 8, 10, 16].includes(b))]
