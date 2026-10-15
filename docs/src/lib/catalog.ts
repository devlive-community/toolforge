// 站点数据类型与加载：catalog.json / release.json 由 generate-data.mjs 生成
import catalogJson from '@/data/catalog.json'
import releaseJson from '@/data/release.json'

export type Lang = 'zh' | 'en'
export const LANGS: Lang[] = ['zh', 'en']

export interface ToolEntry {
  slug: string
  name: Record<Lang, string>
  description: Record<Lang, string>
  category: string
  categoryLabel: Record<Lang, string>
  keywords: string[]
}

export interface ReleaseAssetSet {
  macosApple?: string
  macosIntel?: string
  windowsX64?: string
  windowsArm?: string
  linuxAppImage?: string
  linuxDeb?: string
}

export interface ReleaseData {
  version: string
  url: string
  assets: ReleaseAssetSet
  releases: { tag: string; name: string; date: string; body: string }[]
  fallback?: boolean
}

export const catalog = catalogJson as ToolEntry[]
export const release = releaseJson as ReleaseData

export function getTool(slug: string): ToolEntry | undefined {
  return catalog.find((tool) => tool.slug === slug)
}

export function toolName(tool: ToolEntry, lang: Lang): string {
  return tool.name[lang] || tool.name.en || tool.slug
}

export function toolDescription(tool: ToolEntry, lang: Lang): string {
  return tool.description[lang] || tool.description.en || ''
}

export const REPO_URL = 'https://github.com/devlive-community/toolforge'
export const RELEASES_URL = `https://github.com/${'devlive-community/toolforge'}/releases`
