export interface DraftFactor { name: string; valuesText: string }
export interface Draft { factors: DraftFactor[]; forbiddenText: string }
export interface Limits { maxCases: number; maxCombinations: number; budgetMs: number }
export interface Project {
  mode: 'form' | 'json'
  draft: Draft
  text: string
  seed: string
  limits: Limits
}
export interface Prepared extends Draft { modelText: string; combinations: number }
export type PairState = 'covered' | 'uncovered' | 'infeasible' | 'unknown'
export interface Summary {
  handle: string
  algorithm: string
  seed: string
  status: 'complete' | 'incomplete' | 'unsatisfiable'
  reason: string | null
  factors: { name: string; values: string[] }[]
  modelText: string
  combinations: number
  enumerated: number
  validCombinations: number
  feasibilityComplete: boolean
  caseCount: number
  possiblePairs: number
  feasiblePairs: number
  covered: number
  uncovered: number
  infeasible: number
  unknown: number
  coveragePercent: number | null
}
export interface CaseRow { id: number; cells: string[] }
export interface PairRow { id: number; left: string; leftValue: string; right: string; rightValue: string; state: PairState; caseCount: number }
export interface Page<T> { total: number; items: T[] }
