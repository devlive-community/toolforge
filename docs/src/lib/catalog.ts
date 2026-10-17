// 站点数据类型与加载：catalog.json / release.json 由 generate-data.mjs 生成
import catalogJson from '@/data/catalog.json'
import releaseJson from '@/data/release.json'

export type Lang = 'zh' | 'en'
export const LANGS: Lang[] = ['zh', 'en']

export type ToolStatus = 'released' | 'new' | 'updated' | 'upcoming'

export type ToolChannel = 'builtin' | 'market'

export interface ToolEntry {
  slug: string
  name: Record<Lang, string>
  description: Record<Lang, string>
  category: string
  categoryLabel: Record<Lang, string>
  keywords: string[]
  /** 分发渠道：builtin 随应用内置，market 在应用内工具市场在线安装 */
  channel?: ToolChannel
  /** 插件 manifest 的版本号（market 插件独立发版） */
  version?: string
  /** 发布状态：数据管线对比最新两个版本 tag 的 plugins/ 集合得出，缺失时视为已发布 */
  status?: ToolStatus
  /** status 为 upcoming 且由“有未发布的更新”引起（而非全新工具）时为 true */
  updateOnly?: boolean
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

/**
 * 工具发布状态：
 * - released：早已随历史版本发布
 * - new：随最新版本首次发布（最新 tag 有、上个 tag 没有）
 * - updated：随最新版本更新发布（两个 tag 都有、插件目录内容变化）
 * - upcoming：新工具尚未发布，或插件目录在最新 tag 后有已合入的变化
 * 由 generate-data.mjs 在构建时计算；数据不可用时一律视为 released，避免整站误标。
 */
export function toolStatus(tool: ToolEntry): ToolStatus {
  return tool.status ?? 'released'
}

/** 分发渠道：缺失视为内置（历史数据兼容） */
export function toolChannel(tool: ToolEntry): ToolChannel {
  return tool.channel ?? 'builtin'
}

/** 在线插件的应用内完整 id：官网与安装索引共用的 id 前缀 */
export const PLUGIN_ID_PREFIX = 'org.devlive.toolforge.'

/** 一键安装深链：唤起本地 ToolForge 桌面端并自动安装该在线插件 */
export function toolInstallUrl(slug: string): string {
  return `toolforge://plugin/install/${PLUGIN_ID_PREFIX}${slug}`
}

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
