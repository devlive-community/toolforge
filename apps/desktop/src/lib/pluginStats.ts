import { invoke } from '@tauri-apps/api/core'

export interface PluginDownloadStats {
  pluginId: string
  totalDownloads: string
  versions: { version: string; downloads: string }[]
}

type StatsRequest = (ids: string[]) => Promise<unknown>
const isRecord = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value)
const isCount = (value: unknown): value is string => typeof value === 'string' && /^[0-9]{1,20}$/.test(value)

/** Keep counters as decimal strings; failed or missing responses remain unknown. */
export async function loadDownloadStats(ids: string[], request: StatsRequest = (batch) => invoke('plugin_download_stats', { ids: batch })) {
  const unique = [...new Set(ids)]
  const batches = Array.from({ length: Math.ceil(unique.length / 100) }, (_, index) => unique.slice(index * 100, (index + 1) * 100))
  const results = await Promise.allSettled(batches.map(request))
  const stats = new Map<string, PluginDownloadStats>()
  for (const [index, result] of results.entries()) {
    if (result.status !== 'fulfilled' || !isRecord(result.value) || !Array.isArray(result.value.plugins)) continue
    const requested = new Set(batches[index])
    for (const item of result.value.plugins) {
      if (!isRecord(item) || typeof item.pluginId !== 'string' || !requested.has(item.pluginId) || !isCount(item.totalDownloads) || !Array.isArray(item.versions)) continue
      if (!item.versions.every((version) => isRecord(version) && typeof version.version === 'string' && isCount(version.downloads))) continue
      stats.set(item.pluginId, item as unknown as PluginDownloadStats)
    }
  }
  return stats
}

export function formatDownloadCount(value: string | undefined, locale: string): string | null {
  return isCount(value) ? new Intl.NumberFormat(locale).format(BigInt(value)) : null
}
