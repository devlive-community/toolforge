export interface Source {
  dir: string
  line: number
  section: string
}

export interface Property {
  key: string
  value: string | null
  kind: 'standard' | 'extension' | 'derived'
  invalid: boolean
  source: Source | null
  trace: { value: string; source: Source; unset: boolean }[]
}

export interface TargetResult {
  path: string
  files: string[]
  rootDir: string | null
  incomplete: boolean
  sections: { dir: string; line: number; glob: string; matched: boolean }[]
  properties: Property[]
  unsetStandard: string[]
}

export interface Problem {
  dir: string
  kind: 'invalidLine' | 'pairOutsideSection' | 'invalidRoot' | 'emptySection' | 'invalidGlob'
  line: number
  text: string
}

export interface Output {
  specVersion: string
  targets: TargetResult[]
  problems: Problem[]
}

export interface ConfigInput {
  dir: string
  text: string
}

/** .editorconfig 的显示名 */
export const fileLabel = (dir: string) => (dir ? `${dir}/.editorconfig` : '.editorconfig')
