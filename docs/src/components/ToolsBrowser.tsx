'use client'

import { useEffect, useMemo, useState } from 'react'
import { useSearchParams } from 'next/navigation'
import {
  ArrowLeftRight,
  Calculator,
  ChevronLeft,
  ChevronRight,
  Code2,
  ImageIcon,
  LayoutGrid,
  Monitor,
  Search,
  SearchX,
  Shapes,
  Shield,
  TextCursorInput,
  Wifi,
  X,
  type LucideIcon,
} from 'lucide-react'
import { ToolCard } from './ToolCard'
import { SiteSelect } from './SiteSelect'
import { toolChannel, toolName, toolStatus, type Lang, type ToolChannel, type ToolEntry, type ToolStatus } from '@/lib/catalog'
import { fetchTotalDownloads, pluginStatsId } from '@/lib/stats'

const PAGE_SIZE = 12
const categoryIcons: Record<string, LucideIcon> = {
  calc: Calculator,
  convert: ArrowLeftRight,
  dev: Code2,
  encode: Shield,
  image: ImageIcon,
  network: Wifi,
  other: Shapes,
  life: Shapes,
  graphics: ImageIcon,
  system: Monitor,
  text: TextCursorInput,
}
const copy = {
  zh: {
    search: '搜索工具名称、用途或关键词…',
    clear: '清空搜索',
    categories: '工具分类',
    all: '全部工具',
    count: '个结果',
    source: '工具来源',
    allSources: '全部来源',
    builtin: '应用内置',
    market: '在线插件',
    status: '发布状态',
    more: '更多筛选',
    reset: '重置筛选',
    empty: '换个关键词，再找找看',
    emptyHint: '也可以清空搜索或切换分类。',
    note: '工具在桌面应用中运行。此处浏览功能与使用文档。',
    prev: '上一页',
    next: '下一页',
    pagination: '工具列表分页',
    statuses: { all: '全部状态', released: '已发布', new: '新发布', updated: '已更新', upcoming: '待发布' },
  },
  en: {
    search: 'Search by name, purpose or keyword…',
    clear: 'Clear search',
    categories: 'Categories',
    all: 'All tools',
    count: 'results',
    source: 'Tool source',
    allSources: 'All sources',
    builtin: 'Built-in',
    market: 'Online plugins',
    status: 'Release status',
    more: 'More filters',
    reset: 'Reset filters',
    empty: 'Try another search',
    emptyHint: 'Clear your search or choose another category.',
    note: 'Tools run in the desktop app. Browse features and read their guides here.',
    prev: 'Previous page',
    next: 'Next page',
    pagination: 'Tool list pages',
    statuses: { all: 'All statuses', released: 'Released', new: 'New', updated: 'Updated', upcoming: 'Upcoming' },
  },
} as const

export function ToolsBrowser({ tools, lang }: { tools: ToolEntry[]; lang: Lang }) {
  const params = useSearchParams()
  return (
    <Catalog
      key={`${lang}:${params.toString()}`}
      tools={tools}
      lang={lang}
      initialQuery={(params.get('q') ?? '').slice(0, 512)}
      initialCategory={params.get('category') ?? 'all'}
    />
  )
}
function Catalog({ tools, lang, initialQuery, initialCategory }: { tools: ToolEntry[]; lang: Lang; initialQuery: string; initialCategory: string }) {
  const t = copy[lang]
  const [query, setQuery] = useState(initialQuery)
  const [category, setCategory] = useState(tools.some((tool) => tool.category === initialCategory) ? initialCategory : 'all')
  const [channel, setChannel] = useState<'all' | ToolChannel>('all')
  const [status, setStatus] = useState<'all' | ToolStatus>('all')
  const [page, setPage] = useState(1)
  const [downloads, setDownloads] = useState<Record<string, number>>({})
  useEffect(() => {
    let active = true
    const ids = tools.filter((tool) => toolChannel(tool) === 'market').map((tool) => pluginStatsId(tool.slug))
    fetchTotalDownloads(ids)
      .then((values) => {
        if (active) setDownloads(values)
      })
      .catch(() => {})
    return () => {
      active = false
    }
  }, [tools])
  const categories = useMemo(() => [...new Map(tools.map((tool) => [tool.category, tool.categoryLabel])).entries()], [tools])
  const searched = useMemo(() => {
    const needle = query.trim().toLowerCase()
    return tools.filter(
      (tool) => !needle || [toolName(tool, lang), tool.name.en, tool.description[lang], ...tool.keywords].join(' ').toLowerCase().includes(needle),
    )
  }, [tools, query, lang])
  const scoped = searched.filter(
    (tool) => (channel === 'all' || toolChannel(tool) === channel) && (status === 'all' || (tool.releaseVerified !== false && toolStatus(tool) === status)),
  )
  const filtered = scoped.filter((tool) => category === 'all' || tool.category === category)
  const totalPages = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE))
  const current = Math.min(page, totalPages)
  const start = (current - 1) * PAGE_SIZE
  const pages = [...new Set([1, current - 1, current, current + 1, totalPages])].filter((p) => p > 0 && p <= totalPages).sort((a, b) => a - b)
  const title = categories.find(([id]) => id === category)?.[1][lang] ?? t.all
  const reset = () => {
    setQuery('')
    setCategory('all')
    setChannel('all')
    setStatus('all')
    setPage(1)
  }
  const changeCategory = (value: string) => {
    setCategory(value)
    setPage(1)
  }
  return (
    <>
      <form className="site-search" role="search" onSubmit={(e) => e.preventDefault()}>
        <Search aria-hidden="true" />
        <input
          type="search"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value)
            setPage(1)
          }}
          placeholder={t.search}
          aria-label={t.search}
          maxLength={512}
          autoComplete="off"
        />
        <button
          type="button"
          className="site-icon-btn"
          onClick={() => {
            setQuery('')
            setPage(1)
          }}
          aria-label={t.clear}
        >
          <X aria-hidden="true" />
        </button>
      </form>
      <div className="site-catalog">
        <aside>
          <h2 className="site-sidebar-label">{t.categories}</h2>
          <SiteSelect
            className="site-mobile-category"
            label={t.categories}
            value={category}
            onChange={changeCategory}
            options={[
              { value: 'all', label: `${t.all} · ${scoped.length}` },
              ...categories.map(([id, label]) => ({ value: id, label: `${label[lang]} · ${scoped.filter((tool) => tool.category === id).length}` })),
            ]}
          />
          <nav className="site-categories" aria-label={t.categories}>
            <button className="site-category" type="button" aria-pressed={category === 'all'} onClick={() => changeCategory('all')}>
              <LayoutGrid aria-hidden="true" />
              <span>{t.all}</span>
              <span>{scoped.length}</span>
            </button>
            {categories.map(([id, label]) => {
              const Icon = categoryIcons[id] ?? Shapes
              return (
                <button className="site-category" key={id} type="button" aria-pressed={category === id} onClick={() => changeCategory(id)}>
                  <Icon aria-hidden="true" />
                  <span>{label[lang]}</span>
                  <span>{scoped.filter((tool) => tool.category === id).length}</span>
                </button>
              )
            })}
          </nav>
          <p className="site-side-note">{t.note}</p>
        </aside>
        <section className="site-catalog-results" aria-label={t.all}>
          <div className="site-result-head">
            <h2>
              {title}
              <small aria-live="polite">
                {filtered.length} {t.count}
              </small>
            </h2>
            <SiteSelect<'all' | ToolChannel>
              label={t.source}
              value={channel}
              onChange={(value) => {
                setChannel(value)
                setPage(1)
              }}
              options={[
                { value: 'all', label: t.allSources },
                { value: 'builtin', label: t.builtin },
                { value: 'market', label: t.market },
              ]}
            />
          </div>
          <details className="site-filter-popover">
            <summary>
              {t.more}
              {status !== 'all' && ` · ${t.statuses[status]}`}
            </summary>
            <div className="site-status-options" role="group" aria-label={t.status}>
              {(['all', 'released', 'new', 'updated', 'upcoming'] as const).map((value) => (
                <button
                  type="button"
                  key={value}
                  aria-pressed={status === value}
                  onClick={() => {
                    setStatus(value)
                    setPage(1)
                  }}
                >
                  {t.statuses[value]}
                </button>
              ))}
            </div>
          </details>
          {filtered.length ? (
            <>
              <div className="site-grid site-catalog-grid">
                {filtered.slice(start, start + PAGE_SIZE).map((tool) => (
                  <ToolCard key={tool.slug} tool={tool} lang={lang} downloads={downloads[pluginStatsId(tool.slug)]} />
                ))}
              </div>
              <div className="site-pages">
                <span>
                  {lang === 'zh'
                    ? `显示 ${start + 1}–${Math.min(start + PAGE_SIZE, filtered.length)} / ${filtered.length}`
                    : `${start + 1}–${Math.min(start + PAGE_SIZE, filtered.length)} of ${filtered.length}`}
                </span>
                <nav className="site-page-buttons" aria-label={t.pagination}>
                  <button type="button" disabled={current === 1} aria-label={t.prev} onClick={() => setPage(current - 1)}>
                    <ChevronLeft aria-hidden="true" />
                  </button>
                  {pages.map((p, i) => (
                    <span key={p}>
                      {i > 0 && p - pages[i - 1] > 1 && <span aria-hidden="true">…</span>}
                      <button
                        type="button"
                        aria-current={p === current ? 'page' : undefined}
                        aria-label={lang === 'zh' ? `第 ${p} 页` : `Page ${p}`}
                        onClick={() => setPage(p)}
                      >
                        {p}
                      </button>
                    </span>
                  ))}
                  <button type="button" disabled={current === totalPages} aria-label={t.next} onClick={() => setPage(current + 1)}>
                    <ChevronRight aria-hidden="true" />
                  </button>
                </nav>
              </div>
            </>
          ) : (
            <div className="site-empty">
              <SearchX aria-hidden="true" />
              <h3>{t.empty}</h3>
              <p>{t.emptyHint}</p>
              <button type="button" className="site-secondary site-small" onClick={reset}>
                {t.reset}
              </button>
            </div>
          )}
        </section>
      </div>
    </>
  )
}
