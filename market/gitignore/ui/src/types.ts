export type Category = 'all' | 'language' | 'framework' | 'editor' | 'os' | 'custom'
export type InputTab = 'templates' | 'base' | 'custom'
export type LineEnding = 'lf' | 'crlf'

export interface Template {
  id: string
  name: string
  category: Exclude<Category, 'all'>
  keywords: string[]
  ruleCount: number
}

export interface Catalog {
  templates: Template[]
  source: { url: string; revision: string; license: string; licenseText: string }
}

export interface Generated {
  output: string
  stats: { templateCount: number; ruleCount: number; duplicateCount: number; lineCount: number }
  sections: { id: string; name: string; ruleCount: number }[]
}

export interface CustomTemplate {
  id: string
  name: string
  content: string
}

export interface Library {
  version: 1
  nextId: number
  templates: CustomTemplate[]
}

export interface LibraryResult {
  library: Library
  templateId?: string
}

export type LibraryMutation =
  | { operation: 'save'; id?: string; name: string; content: string }
  | { operation: 'delete'; id: string }
