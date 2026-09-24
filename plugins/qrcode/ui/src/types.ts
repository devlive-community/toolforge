export type Ecc = 'L' | 'M' | 'Q' | 'H'

export interface Generated {
  image: string
  version: number
  modules: number
  pixels: number
  bytes: number
}

export interface Decoded {
  text: string
  version: number
  ecc: Ecc
}

export const ECC_LEVELS: Ecc[] = ['L', 'M', 'Q', 'H']
