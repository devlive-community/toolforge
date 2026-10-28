/** 在线插件市场的筛选与排序（纯函数，便于测试） */

/** 「新上架」「近期更新」的时间范围（天） */
export const RECENT_DAYS = 14

export type MarketFilter = 'all' | 'new' | 'updated' | 'updatable' | 'installed' | 'notInstalled'
export type MarketSort = 'updated' | 'published' | 'name' | 'downloads'

/** 参与筛选排序的条目字段（与插件索引一致，时间为 UTC RFC 3339，旧索引可能没有） */
export interface MarketItem {
  id: string
  name: string
  description: string
  category: string
  publishedAt?: string | null
  updatedAt?: string | null
  installed: boolean
  updatable: boolean
  downloads: bigint | null
}

const DAY_MS = 24 * 60 * 60 * 1000

const time = (value: string | null | undefined) => {
  const ms = value ? Date.parse(value) : Number.NaN
  return Number.isNaN(ms) ? null : ms
}

/** 首次发布在最近 RECENT_DAYS 天内 */
export function isNew(item: MarketItem, now: number): boolean {
  const published = time(item.publishedAt)
  return published !== null && now - published <= RECENT_DAYS * DAY_MS
}

/** 已有旧版本、最近 RECENT_DAYS 天内发布了新版本（新上架的不重复算作更新） */
export function isRecentlyUpdated(item: MarketItem, now: number): boolean {
  const updated = time(item.updatedAt)
  const published = time(item.publishedAt)
  return updated !== null && published !== null && updated > published && now - updated <= RECENT_DAYS * DAY_MS && !isNew(item, now)
}

export function matchesFilter(item: MarketItem, filter: MarketFilter, now: number): boolean {
  switch (filter) {
    case 'new':
      return isNew(item, now)
    case 'updated':
      return isRecentlyUpdated(item, now)
    case 'updatable':
      return item.updatable
    case 'installed':
      return item.installed
    case 'notInstalled':
      return !item.installed
    default:
      return true
  }
}

/** 关键词按空白拆分，全部命中名称、说明或 ID 才算匹配（不区分大小写） */
export function matchesQuery(item: MarketItem, query: string): boolean {
  const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean)
  if (words.length === 0) return true
  const haystack = `${item.name} ${item.description} ${item.id}`.toLowerCase()
  return words.every((word) => haystack.includes(word))
}

/** 排序：没有时间或下载量的条目排在后面，其余相同时按名称 */
export function compareItems(sort: MarketSort, locale: string) {
  const byName = (a: MarketItem, b: MarketItem) => a.name.localeCompare(b.name, locale)
  const desc = (x: number | null, y: number | null) => (x === y ? 0 : x === null ? 1 : y === null ? -1 : y - x)
  return (a: MarketItem, b: MarketItem) => {
    let order = 0
    if (sort === 'updated') order = desc(time(a.updatedAt ?? a.publishedAt), time(b.updatedAt ?? b.publishedAt))
    else if (sort === 'published') order = desc(time(a.publishedAt), time(b.publishedAt))
    else if (sort === 'downloads') {
      const x = a.downloads
      const y = b.downloads
      order = x === y ? 0 : x === null ? 1 : y === null ? -1 : y > x ? 1 : -1
    }
    return order || byName(a, b)
  }
}

/** 各筛选项的数量 */
export function countFilters(items: MarketItem[], now: number): Record<MarketFilter, number> {
  const filters: MarketFilter[] = ['all', 'new', 'updated', 'updatable', 'installed', 'notInstalled']
  return Object.fromEntries(filters.map((f) => [f, items.filter((item) => matchesFilter(item, f, now)).length])) as Record<MarketFilter, number>
}
