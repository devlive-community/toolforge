export type Part = 'stem' | 'ext' | 'full'
export type Place = 'start' | 'end' | 'index'
export type Sort = 'added' | 'name' | 'modified' | 'size'

export type Rule =
  | { kind: 'replace'; find: string; replace: string; regex: boolean; caseSensitive: boolean; part: Part }
  | { kind: 'insert'; text: string; place: Place; index: number; part: Part }
  | { kind: 'remove'; place: Place; index: number; count: number; part: Part }
  | { kind: 'case'; mode: 'lower' | 'upper' | 'title' | 'sentence'; part: Part }
  | { kind: 'number'; start: number; step: number; pad: number; place: 'start' | 'end'; separator: string }
  | { kind: 'template'; pattern: string }
  | { kind: 'extension'; mode: 'lower' | 'upper' | 'set' | 'remove'; value: string }
  | { kind: 'clean'; collapseSpaces: boolean; spacesTo: string; trim: boolean; removeIllegal: boolean }

export type Kind = Rule['kind']
export type Step = Rule & { id: string; enabled: boolean }

export const KINDS: Kind[] = ['replace', 'insert', 'remove', 'case', 'number', 'template', 'extension', 'clean']

export function newRule(kind: Kind): Rule {
  switch (kind) {
    case 'replace':
      return { kind, find: '', replace: '', regex: false, caseSensitive: false, part: 'stem' }
    case 'insert':
      return { kind, text: '', place: 'start', index: 0, part: 'stem' }
    case 'remove':
      return { kind, place: 'start', index: 0, count: 1, part: 'stem' }
    case 'case':
      return { kind, mode: 'lower', part: 'stem' }
    case 'number':
      return { kind, start: 1, step: 1, pad: 3, place: 'start', separator: '_' }
    case 'template':
      return { kind, pattern: '{name}' }
    case 'extension':
      return { kind, mode: 'lower', value: '' }
    case 'clean':
      return { kind, collapseSpaces: true, spacesTo: '', trim: true, removeIllegal: true }
  }
}

export type Status = 'unchanged' | 'ok' | 'conflict' | 'invalid' | 'missing'

export interface Item {
  path: string
  from: string
  to: string
  status: Status
  reason: string | null
}

export interface Plan {
  items: Item[]
  changes: number
  problems: number
}

export interface Move {
  from: string
  to: string
}

export interface Batch {
  journal: Move[]
  at: number
}
