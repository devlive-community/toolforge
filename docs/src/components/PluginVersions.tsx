'use client'
import { useState } from 'react'
import Link from 'next/link'
import { ArrowUpRight, ChevronLeft, ChevronRight } from 'lucide-react'
import { ToolIcon } from './ToolCard'
import { canInstall, toolChannel, toolDescription, toolName, type Lang, type ToolEntry } from '@/lib/catalog'

export function PluginVersions({ tools, lang }: { tools: ToolEntry[]; lang: Lang }) {
  const [page, setPage] = useState(1)
  const published = tools.filter((tool) => canInstall(tool))
  const unknown = tools.some((tool) => toolChannel(tool) === 'market' && tool.releaseVerified === false)
  const pages = Math.max(1, Math.ceil(published.length / 10))
  const numbers = [...new Set([1, page - 1, page, page + 1, pages])].filter((n) => n > 0 && n <= pages).sort((a, b) => a - b)
  const zh = lang === 'zh'
  return (
    <>
      {unknown && (
        <div className="site-note site-note-warn">
          {zh
            ? '暂时无法确认插件发布信息，请稍后重试或在应用内工具市场查看。'
            : 'Plugin releases could not be verified. Try later or use the in-app tool market.'}
        </div>
      )}
      {published.slice((page - 1) * 10, page * 10).map((tool) => (
        <article className="site-plugin-release" key={tool.slug}>
          <ToolIcon slug={tool.slug} hasIcon={tool.hasIcon} />
          <div>
            <h2>
              <Link href={`/${lang}/tools/${tool.slug}/`}>{toolName(tool, lang)}</Link> <span className="site-tag">v{tool.publishedVersion}</span>
            </h2>
            <p>{toolDescription(tool, lang)}</p>
          </div>
          <Link href={`/${lang}/tools/${tool.slug}/`}>
            {zh ? '查看文档' : 'Read guide'}
            <ArrowUpRight aria-hidden="true" />
          </Link>
        </article>
      ))}
      {published.length === 0 && !unknown && (
        <div className="site-empty">{zh ? '当前没有已发布的插件版本。' : 'No published plugin versions are available.'}</div>
      )}
      {pages > 1 && (
        <nav className="site-pages" aria-label={zh ? '插件版本分页' : 'Plugin version pages'}>
          <span>{zh ? `共 ${published.length} 个插件` : `${published.length} plugins`}</span>
          <div className="site-page-buttons">
            <button type="button" disabled={page === 1} onClick={() => setPage(page - 1)} aria-label={zh ? '上一页' : 'Previous page'}>
              <ChevronLeft aria-hidden="true" />
            </button>
            {numbers.map((n, i) => (
              <span key={n}>
                {i > 0 && n - numbers[i - 1] > 1 && <span>…</span>}
                <button type="button" aria-current={n === page ? 'page' : undefined} onClick={() => setPage(n)} aria-label={zh ? `第 ${n} 页` : `Page ${n}`}>
                  {n}
                </button>
              </span>
            ))}
            <button type="button" disabled={page === pages} onClick={() => setPage(page + 1)} aria-label={zh ? '下一页' : 'Next page'}>
              <ChevronRight aria-hidden="true" />
            </button>
          </div>
        </nav>
      )}
    </>
  )
}
