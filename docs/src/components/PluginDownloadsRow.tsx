'use client'

// 详情页「工具信息」面板的下载次数行：浏览器端从统计服务拉取（静态站点无法
// 构建时生成）。拉取失败或该插件尚无统计记录时不渲染，与列表页的降级策略一致。

import { useEffect, useState } from 'react'
import { type Lang } from '@/lib/catalog'
import { fetchTotalDownloads, formatDownloadCount, pluginStatsId } from '@/lib/stats'

export function PluginDownloadsRow({ slug, lang }: { slug: string; lang: Lang }) {
  const [downloads, setDownloads] = useState<number | null>(null)
  useEffect(() => {
    let active = true
    fetchTotalDownloads([pluginStatsId(slug)])
      .then((totals) => {
        if (!active) return
        const value = totals[pluginStatsId(slug)]
        if (value !== undefined) setDownloads(value)
      })
      .catch(() => {})
    return () => {
      active = false
    }
  }, [slug])
  if (downloads === null) return null
  return (
    <div className="site-info-row">
      <span>{lang === 'zh' ? '下载次数' : 'Downloads'}</span>
      <span>{formatDownloadCount(downloads, lang)}</span>
    </div>
  )
}
