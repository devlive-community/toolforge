export type Draft = '2020-12' | '7'
export interface Diagnostic {
  target: 'instance' | 'schema'
  code: string
  instancePath: string
  schemaPath: string
  keyword: string
  detail: string
  line: number | null
  column: number | null
}
export interface Report {
  valid: boolean
  schemaValid: boolean
  diagnostics: Diagnostic[]
  truncated: boolean
  elapsedMs: number
}
export interface Submission {
  instance: string
  schema: string
  draft: Draft
  validateFormats: boolean
}
export type InferSplit = 'document' | 'array' | 'lines'
export interface InferResult {
  schema: string
  samples: number
}
