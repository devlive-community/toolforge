import history from '@/data/plugin-history.json'

export interface PluginChange {
  type: 'feat' | 'fix' | 'perf'
  text: string
  hash: string
}
export interface PluginVersion {
  version: string
  date: string
  pending: boolean
  changes: PluginChange[]
}

/** 插件的版本历史（从新到旧）；构建时没有完整 git 历史则为空 */
export function getPluginHistory(slug: string): PluginVersion[] {
  return (history as Record<string, PluginVersion[]>)[slug] ?? []
}
