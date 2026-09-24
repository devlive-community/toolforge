export interface Generated {
  passwords: string[]
  poolSize: number
  entropy: number
}

export interface Report {
  length: number
  classes: { upper: boolean; lower: boolean; digits: boolean; symbols: boolean; other: boolean }
  poolSize: number
  entropy: number
  score: number
  crackTime: { unit: string; value: number }
  warnings: string[]
}
