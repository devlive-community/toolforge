import documents from '@/data/tool-docs.json'
import info from '@/data/site-info.json'
import type { Lang } from './catalog'

export interface ToolFunction {
  name: string
  expose: ('cli' | 'agent')[]
  summaries: Record<Lang, string>
  effect: 'read' | 'write' | 'external'
  uses: string[]
  confirm: boolean
  task: boolean
  examples: string[]
  params: { name: string; type: string; required: boolean; descriptions: Record<Lang, string>; sensitive: boolean }[]
}
export interface ToolDocumentation {
  permissions: string[]
  minAppVersion: string
  platforms: string[]
  sensitive: boolean
  help: Record<Lang, string[]>
  functions: ToolFunction[]
}
export function getToolDocumentation(slug: string): ToolDocumentation {
  return (
    (documents as Record<string, ToolDocumentation>)[slug] ?? {
      permissions: [],
      minAppVersion: '',
      platforms: [],
      sensitive: false,
      help: { zh: [], en: [] },
      functions: [],
    }
  )
}
export const agentReleased = info.agentReleased
