'use client'

import { useMemo, useState, type ReactNode } from 'react'
import {
  ArrowLeftRight, Calculator, ChevronLeft, ChevronRight, Code2, ImageIcon, Layers,
  LayoutGrid, Monitor, Search, SearchX, Shield, TextCursorInput, Wifi, X,
  type LucideIcon,
} from 'lucide-react'
import { ToolCard } from '@/components/ToolCard'
import { toolName, toolStatus, type Lang, type ToolEntry, type ToolStatus } from '@/lib/catalog'
import styles from './ToolsBrowser.module.css'

const PAGE_SIZE = 15
const statuses: Array<'all' | ToolStatus> = ['all', 'released', 'new', 'updated', 'upcoming']
const categoryIcons: Record<string, LucideIcon> = {
  calc: Calculator, convert: ArrowLeftRight, dev: Code2, encode: Shield,
  image: ImageIcon, network: Wifi, other: Layers, system: Monitor, text: TextCursorInput,
}

const copy = {
  zh: {
    search: '搜索工具名称、功能或关键词…', clearSearch: '清空搜索',
    categories: '工具分类', allTools: '全部工具',
    categoriesNote: '在桌面端打开工具，所有数据处理均在本机完成。',
    filterLabel: '发布状态',
    statuses: { all: '全部状态', released: '已发布', new: '新发布', updated: '已更新', upcoming: '待发布' },
    result: '个工具', resultsLabel: '筛选结果',
    empty: '暂时没有找到匹配的工具', emptyBody: '试试其他关键词，或调整分类与发布状态。',
    reset: '重置所有筛选', previous: '上一页', next: '下一页', pagination: '工具列表分页',
    page: '第 {page} 页', range: '显示 {start}–{end} 项，共 {total} 个工具', pageSummary: '第 {page} / {total} 页',
  },
  en: {
    search: 'Search by name, feature or keyword…', clearSearch: 'Clear search',
    categories: 'CATEGORIES', allTools: 'All tools',
    categoriesNote: 'Open tools in the desktop app. All processing stays on your device.',
    filterLabel: 'Release status',
    statuses: { all: 'All statuses', released: 'Released', new: 'New', updated: 'Updated', upcoming: 'Upcoming' },
    result: 'tools', resultsLabel: 'Filtered results',
    empty: 'No matching tools yet', emptyBody: 'Try a different keyword, category or release status.',
    reset: 'Reset all filters', previous: 'Previous page', next: 'Next page', pagination: 'Tool list pagination',
    page: 'Page {page}', range: 'Showing {start}–{end} of {total} tools', pageSummary: 'Page {page} of {total}',
  },
} satisfies Record<Lang, {
  search: string; clearSearch: string; categories: string; allTools: string; categoriesNote: string
  filterLabel: string; statuses: Record<'all' | ToolStatus, string>; result: string; resultsLabel: string
  empty: string; emptyBody: string; reset: string; previous: string; next: string
  pagination: string; page: string; range: string; pageSummary: string
}>

export function ToolsBrowser({ tools, lang }: { tools: ToolEntry[]; lang: Lang }) {
  const t = copy[lang]
  const [category, setCategory] = useState('all')
  const [status, setStatus] = useState<'all' | ToolStatus>('all')
  const [query, setQuery] = useState('')
  const [page, setPage] = useState(1)

  const categories = useMemo(() => {
    const seen = new Map<string, ToolEntry['categoryLabel']>()
    for (const tool of tools) seen.set(tool.category, tool.categoryLabel)
    return [...seen.entries()].map(([slug, label]) => ({ slug, label }))
  }, [tools])

  const searched = useMemo(() => {
    const needle = query.trim().toLowerCase()
    if (!needle) return tools
    return tools.filter((tool) => [
      toolName(tool, lang), tool.name.en, tool.description[lang], tool.description.en, ...tool.keywords,
    ].join(' ').toLowerCase().includes(needle))
  }, [tools, query, lang])

  // Counts remain useful while combining search, category and release status.
  const categoryCounts = useMemo(() => {
    const matched = searched.filter((tool) => status === 'all' || toolStatus(tool) === status)
    const counts: Record<string, number> = { all: matched.length }
    for (const tool of matched) counts[tool.category] = (counts[tool.category] ?? 0) + 1
    return counts
  }, [searched, status])

  const statusCounts = useMemo(() => {
    const matched = searched.filter((tool) => category === 'all' || tool.category === category)
    const counts: Record<string, number> = { all: matched.length }
    for (const tool of matched) {
      const toolReleaseStatus = toolStatus(tool)
      counts[toolReleaseStatus] = (counts[toolReleaseStatus] ?? 0) + 1
    }
    return counts
  }, [searched, category])

  const filtered = useMemo(() => searched.filter((tool) =>
    (category === 'all' || tool.category === category) &&
    (status === 'all' || toolStatus(tool) === status)
  ), [searched, category, status])

  const totalPages = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE))
  const currentPage = Math.min(page, totalPages)
  const start = (currentPage - 1) * PAGE_SIZE
  const visibleTools = filtered.slice(start, start + PAGE_SIZE)
  const selectedCategory = categories.find((item) => item.slug === category)
  const categoryTitle = selectedCategory ? selectedCategory.label[lang] : t.allTools
  const hasFilters = query.trim() !== '' || category !== 'all' || status !== 'all'

  const reset = () => {
    setQuery('')
    setCategory('all')
    setStatus('all')
    setPage(1)
  }

  return (
    <div className={styles.browser}>
      <div className={styles.searchField}>
        <Search size={20} aria-hidden="true" />
        <input id="tool-search" type="search" value={query}
          onChange={(event) => { setQuery(event.target.value); setPage(1) }}
          placeholder={t.search} aria-label={t.search} autoComplete="off" />
        {query !== '' && <button type="button" className={styles.clearSearch} aria-label={t.clearSearch} onClick={() => { setQuery(''); setPage(1) }}><X size={17} /></button>}
      </div>

      <div className={styles.layout}>
        <aside className={styles.sidebar} aria-labelledby="tool-categories">
          <h2 id="tool-categories" className={styles.sidebarHeading}>{t.categories}</h2>
          <div className={styles.categories}>
            <CategoryButton active={category === 'all'} count={categoryCounts.all ?? 0} icon={LayoutGrid} onClick={() => { setCategory('all'); setPage(1) }}>{t.allTools}</CategoryButton>
            {categories.map((item) => <CategoryButton key={item.slug} active={category === item.slug} count={categoryCounts[item.slug] ?? 0} icon={categoryIcons[item.slug] ?? Layers} onClick={() => { setCategory(item.slug); setPage(1) }}>{item.label[lang]}</CategoryButton>)}
          </div>
          <p className={styles.sidebarNote}><Shield size={16} aria-hidden="true" />{t.categoriesNote}</p>
        </aside>

        <section className={styles.results} aria-label={t.resultsLabel}>
          <div className={styles.resultHeading}>
            <div className={styles.resultTitle}><h2>{categoryTitle}</h2><span aria-live="polite">{filtered.length} {t.result}</span></div>
            {hasFilters && <button type="button" className={styles.reset} onClick={reset}>{t.reset}<X size={12} aria-hidden="true" /></button>}
          </div>
          <div className={styles.statuses} role="group" aria-label={t.filterLabel}>
            {statuses.map((item) => <button key={item} type="button" aria-pressed={status === item} className={`${styles.status} ${status === item ? styles.statusActive : ''}`} onClick={() => { setStatus(item); setPage(1) }}>{item !== 'all' && <span className={`${styles.statusDot} ${item === 'upcoming' ? styles.upcomingDot : ''}`} aria-hidden="true" />}{t.statuses[item]}<span className={styles.statusCount}>{statusCounts[item] ?? 0}</span></button>)}
          </div>

          {filtered.length === 0 ? (
            <div className={styles.empty}>
              <div className={styles.emptyIcon}><SearchX size={27} aria-hidden="true" /></div>
              <h3>{t.empty}</h3><p>{t.emptyBody}</p>
              <button type="button" onClick={reset}>{t.reset}<ChevronRight size={15} aria-hidden="true" /></button>
            </div>
          ) : (
            <>
              <div className={styles.grid}>{visibleTools.map((tool) => <ToolCard key={tool.slug} tool={tool} lang={lang} />)}</div>
              <div className={styles.paginationFooter}>
                <p>{t.range.replace('{start}', String(start + 1)).replace('{end}', String(start + visibleTools.length)).replace('{total}', String(filtered.length))}</p>
                <nav className={styles.pagination} aria-label={t.pagination}>
                  <button type="button" className={styles.pageButton} aria-label={t.previous} disabled={currentPage === 1} onClick={() => setPage(currentPage - 1)}><ChevronLeft size={16} /></button>
                  {Array.from({ length: totalPages }, (_, index) => index + 1).map((pageNumber) => <button type="button" key={pageNumber} className={`${styles.pageButton} ${currentPage === pageNumber ? styles.pageActive : ''}`} aria-label={t.page.replace('{page}', String(pageNumber))} aria-current={currentPage === pageNumber ? 'page' : undefined} onClick={() => setPage(pageNumber)}>{pageNumber}</button>)}
                  <button type="button" className={styles.pageButton} aria-label={t.next} disabled={currentPage === totalPages} onClick={() => setPage(currentPage + 1)}><ChevronRight size={16} /></button>
                </nav>
                <span className={styles.pageSummary}>{t.pageSummary.replace('{page}', String(currentPage)).replace('{total}', String(totalPages))}</span>
              </div>
            </>
          )}
        </section>
      </div>
    </div>
  )
}

function CategoryButton({ active, count, icon: Icon, onClick, children }: {
  active: boolean; count: number; icon: LucideIcon; onClick: () => void; children: ReactNode
}) {
  return <button type="button" className={`${styles.category} ${active ? styles.categoryActive : ''}`} aria-pressed={active} onClick={onClick}><Icon size={16} aria-hidden="true" /><span>{children}</span><span className={styles.categoryCount}>{count}</span></button>
}
