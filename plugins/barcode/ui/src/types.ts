export type Format = 'ean13' | 'ean8' | 'upcA' | 'itf' | 'code128' | 'code39' | 'code93' | 'codabar' | 'dataMatrix' | 'pdf417' | 'aztec'

export const FORMATS: Format[] = ['ean13', 'ean8', 'upcA', 'itf', 'code128', 'code39', 'code93', 'codabar', 'dataMatrix', 'pdf417', 'aztec']

export const MATRIX_FORMATS = new Set<Format>(['dataMatrix', 'pdf417', 'aztec'])

/** 切换码制时的示例内容 */
export const SAMPLES: Record<Format, string> = {
  ean13: '690123456789',
  ean8: '9638507',
  upcA: '03600029145',
  itf: '1540014128876',
  code128: 'TF-2026-0001',
  code39: 'TOOLFORGE-42',
  code93: 'CODE93',
  codabar: 'A40156B',
  dataMatrix: 'https://github.com/devlive-community/toolforge',
  pdf417: 'ToolForge PDF417',
  aztec: 'ToolForge Aztec',
}

export interface Generated {
  svg: string
  width: number
  height: number
  content: string
  checkDigit: string | null
  notes: string[]
}

export interface Decoded {
  format: string
  text: string
}
