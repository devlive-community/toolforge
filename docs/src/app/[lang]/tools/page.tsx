import { Suspense } from 'react'
import Link from 'next/link'
import { ChevronRight } from 'lucide-react'
import { ToolsBrowser } from '@/components/ToolsBrowser'
import { ToolCard } from '@/components/ToolCard'
import { catalog, type Lang } from '@/lib/catalog'
import { siteCopy } from '@/lib/site-copy'
import { pageMetadata } from '@/lib/metadata'

export async function generateMetadata({ params }: { params: Promise<{ lang: Lang }> }) {
  const { lang } = await params
  return pageMetadata(lang, 'tools/', `${siteCopy[lang].tools} — ToolForge`, siteCopy[lang].toolsBody)
}
export default async function ToolsPage({ params }: { params: Promise<{ lang: Lang }> }) {
  const { lang } = await params
  const t = siteCopy[lang]
  return (
    <div className="site-wrap">
      <header className="site-page-heading">
        <nav className="site-breadcrumb" aria-label={lang === 'zh' ? '面包屑导航' : 'Breadcrumb'}>
          <Link href={`/${lang}/`}>{t.home}</Link>
          <ChevronRight aria-hidden="true" />
          <span aria-current="page">{t.tools}</span>
        </nav>
        <h1>{t.toolsTitle}</h1>
        <p>{t.toolsBody}</p>
      </header>
      <Suspense
        fallback={
          <div className="site-grid" aria-label={t.tools}>
            {catalog.slice(0, 8).map((tool) => (
              <ToolCard key={tool.slug} tool={tool} lang={lang} />
            ))}
          </div>
        }
      >
        <ToolsBrowser tools={catalog} lang={lang} />
      </Suspense>
    </div>
  )
}
