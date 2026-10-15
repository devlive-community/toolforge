import Link from 'next/link'
import { toolName, type Lang, type ToolEntry } from '@/lib/catalog'
import { StatusBadge } from '@/components/StatusBadge'

// 工具卡片：目录页与首页精选共用
export function ToolCard({ tool, lang }: { tool: ToolEntry; lang: Lang }) {
  return (
    <Link
      href={`/${lang}/tools/${tool.slug}/`}
      className="group flex min-w-0 flex-col gap-2.5 rounded-lg border border-line bg-panel p-4 transition-all hover:-translate-y-0.5 hover:border-line-strong hover:shadow-lg focus-visible:outline-2 focus-visible:outline-accent"
    >
      <div className="flex items-center gap-3">
        <img
          src={`/icons/${tool.slug}.svg`}
          alt=""
          width={36}
          height={36}
          loading="lazy"
          className="size-9 shrink-0 rounded-lg border border-line bg-bg-subtle p-1.5 transition-transform group-hover:scale-105"
        />
        <div className="min-w-0">
          <p className="truncate text-[14px] font-medium text-fg">{toolName(tool, lang)}</p>
          <p className="font-display text-[10px] uppercase tracking-wider text-fg-subtle">
            {tool.categoryLabel[lang]}
          </p>
        </div>
        <StatusBadge tool={tool} lang={lang} />
      </div>
      <p className="line-clamp-2 text-xs leading-relaxed text-fg-muted">
        {tool.description[lang] || tool.description.en}
      </p>
    </Link>
  )
}
