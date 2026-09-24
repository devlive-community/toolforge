export interface CategoryInfo {
  id: string
  base: string
  units: { id: string; symbol: string }[]
}

export interface Converted {
  category: string
  input: string
  results: { unit: string; symbol: string; value: string }[]
}
