import Link from 'next/link'
import { ArrowUpRight, Puzzle } from 'lucide-react'
import { canInstall, toolChannel, toolDescription, toolName, type Lang, type ToolEntry } from '@/lib/catalog'
import { formatDownloadCount } from '@/lib/stats'
import { StatusBadge } from './StatusBadge'
import { InstallButton } from './InstallButton'

export function ToolIcon({ slug, hasIcon = true }: { slug: string; hasIcon?: boolean }) {
  return (
    <span className="site-tool-icon">
      {hasIcon ? (
        <span className="site-tool-glyph" aria-hidden="true" style={{ maskImage: `url(/icons/${slug}.svg)`, WebkitMaskImage: `url(/icons/${slug}.svg)` }} />
      ) : (
        <Puzzle aria-hidden="true" />
      )}
    </span>
  )
}

export function ToolCard({ tool, lang, downloads }: { tool: ToolEntry; lang: Lang; downloads?: number }) {
  const market = toolChannel(tool) === 'market'
  return (
    <article className="site-tool">
      <Link href={`/${lang}/tools/${tool.slug}/`} className="site-tool-link" aria-label={toolName(tool, lang)} />
      <div className="site-tool-top">
        <ToolIcon slug={tool.slug} hasIcon={tool.hasIcon} />
        <span className="site-tag">{market ? (lang === 'zh' ? '在线插件' : 'Online plugin') : lang === 'zh' ? '应用内置' : 'Built-in'}</span>
      </div>
      <h3 className="site-tool-title">{toolName(tool, lang)}</h3>
      <p>{toolDescription(tool, lang)}</p>
      <div className="site-tool-bottom">
        <span>{tool.categoryLabel[lang]}</span>
        <StatusBadge tool={tool} lang={lang} />
        {canInstall(tool) ? <InstallButton slug={tool.slug} lang={lang} variant="card" /> : <ArrowUpRight aria-hidden="true" />}
      </div>
      {downloads !== undefined && (
        <span className="site-tool-downloads">
          {formatDownloadCount(downloads, lang)} {lang === 'zh' ? '次下载' : 'downloads'}
        </span>
      )}
    </article>
  )
}
