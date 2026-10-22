// 统计服务公开接口。站点是纯静态导出（GitHub Pages），下载次数只能在浏览器端
// 拉取：统计服务对公开统计端点返回 Access-Control-Allow-Origin: *，允许跨域读取。
// 拉取失败时静默降级，卡片不展示下载次数。

import { PLUGIN_ID_PREFIX, type Lang } from '@/lib/catalog'

export const STATS_BASE_URL = 'https://stats.toolforge.devlive.org'

export type DownloadTotals = Record<string, number>

/** 批量拉取插件总下载次数：GET /v1/stats?ids=org.devlive.toolforge.a,... */
export async function fetchTotalDownloads(ids: string[]): Promise<DownloadTotals> {
  if (ids.length === 0) return {}
  const query = encodeURIComponent(ids.join(','))
  const response = await fetch(`${STATS_BASE_URL}/v1/stats?ids=${query}`)
  if (!response.ok) throw new Error(`stats API unavailable (${response.status})`)
  const data = (await response.json()) as { plugins?: { pluginId: string; totalDownloads: string }[] }
  const totals: DownloadTotals = {}
  for (const item of data.plugins ?? []) {
    const parsed = Number.parseInt(item.totalDownloads, 10)
    if (Number.isFinite(parsed) && parsed >= 0) totals[item.pluginId] = parsed
  }
  return totals
}

/** 在线插件的统计 id：与 catalog 的 slug 对应 */
export function pluginStatsId(slug: string): string {
  return `${PLUGIN_ID_PREFIX}${slug}`
}

/** 紧凑计数展示：zh 走「万」，en 走 K/M（项目 TS lib 不含 ES2020 Intl 选项，手写格式化） */
export function formatDownloadCount(value: number, lang: Lang): string {
  if (lang === 'zh') {
    if (value >= 10000) {
      const wan = Math.round((value / 10000) * 10) / 10
      return `${wan}万`
    }
    return String(value)
  }
  if (value >= 1000000) {
    const million = Math.round((value / 1000000) * 10) / 10
    return `${million}M`
  }
  if (value >= 1000) {
    const thousand = Math.round((value / 1000) * 10) / 10
    return `${thousand}K`
  }
  return String(value)
}
