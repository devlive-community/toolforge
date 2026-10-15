'use client'

import { useMemo, useState } from 'react'
import { Search } from 'lucide-react'
import { ToolCard } from '@/components/ToolCard'
import { toolName, toolStatus, type Lang, type ToolEntry, type ToolStatus } from '@/lib/catalog'
import { dict } from '@/lib/dict'

// 工具目录：分类筛选 + 发布状态筛选 + 关键词搜索（纯客户端，静态托管友好）
export function ToolsBrowser({ tools, lang }: { tools: ToolEntry[]; lang: Lang }) {
  const t = dict.toolsPage
  const [category, setCategory] = useState<string>('all')
  const [status, setStatus] = useState<'all' | ToolStatus>('all')
  const [query, setQuery] = useState('')

  const categories = useMemo(() => {
    const seen = new Map<string, { zh: string; en: string }>()
    for (const tool of tools) seen.set(tool.category, tool.categoryLabel)
    return [...seen.entries()].map(([slug, label]) => ({ slug, ...label }))
  }, [tools])

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase()
    return tools.filter((tool) => {
      if (category !== 'all' && tool.category !== category) return false
      if (status !== 'all' && toolStatus(tool) !== status) return false
      if (!needle) return true
      const haystack = [toolName(tool, lang), tool.description[lang], tool.description.en, ...tool.keywords]
        .join(' ')
        .toLowerCase()
      return haystack.includes(needle)
    })
  }, [tools, category, status, query, lang])

  return (
    <div className="space-y-5">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
        <div className="relative sm:w-72">
          <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-fg-subtle" />
          <input
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t.search[lang]}
            aria-label={t.search[lang]}
            className="h-10 w-full rounded-lg border border-line bg-panel pl-9 pr-3 text-sm text-fg outline-none transition-colors placeholder:text-fg-subtle focus:border-accent focus-visible:outline-2 focus-visible:outline-accent"
          />
        </div>
        <p className="text-xs text-fg-subtle tabular-nums sm:ml-auto">
          {filtered.length} / {tools.length} {t.count[lang]}
        </p>
      </div>

      <div className="flex flex-wrap gap-1.5">
        <Chip active={category === 'all'} onClick={() => setCategory('all')}>
          {t.all[lang]}
        </Chip>
        {categories.map((item) => (
          <Chip key={item.slug} active={category === item.slug} onClick={() => setCategory(item.slug)}>
            {item[lang]}
          </Chip>
        ))}
      </div>

      <div className="flex flex-wrap gap-1.5">
        <Chip active={status === 'all'} onClick={() => setStatus('all')}>
          {t.statusAll[lang]}
        </Chip>
        <Chip active={status === 'released'} onClick={() => setStatus('released')}>
          {t.statusReleased[lang]}
        </Chip>
        <Chip active={status === 'new'} onClick={() => setStatus('new')}>
          {t.statusNew[lang]}
        </Chip>
        <Chip active={status === 'upcoming'} onClick={() => setStatus('upcoming')}>
          {t.statusUpcoming[lang]}
        </Chip>
      </div>

      {filtered.length === 0 ? (
        <p className="rounded-lg border border-dashed border-line p-10 text-center text-sm text-fg-subtle">
          {t.empty[lang]}
        </p>
      ) : (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {filtered.map((tool) => (
            <ToolCard key={tool.slug} tool={tool} lang={lang} />
          ))}
        </div>
      )}
    </div>
  )
}

function Chip({ active, onClick, children }: { active: boolean; onClick: () => void; children: string }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`h-8 rounded-full border px-3.5 text-xs transition-colors focus-visible:outline-2 focus-visible:outline-accent ${
        active
          ? 'border-accent bg-accent-soft font-medium text-accent-strong'
          : 'border-line text-fg-muted hover:border-line-strong hover:text-fg'
      }`}
    >
      {children}
    </button>
  )
}
