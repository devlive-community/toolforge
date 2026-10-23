import Link from 'next/link'
import { notFound } from 'next/navigation'
import { ArrowRight, ChevronRight, Puzzle } from 'lucide-react'
import { DevMarkdown } from '@/components/DevMarkdown'
import { devArticle, devCopy, devTopics, type DevTopic } from '@/lib/dev-docs'
import type { Lang } from '@/lib/catalog'
import { pageMetadata } from '@/lib/metadata'
import { siteCopy } from '@/lib/site-copy'

export function generateStaticParams() {
  return (['zh', 'en'] as const).flatMap((lang) => devTopics.map((topic) => ({ lang, topic })))
}
export async function generateMetadata({ params }: { params: Promise<{ lang: Lang; topic: string }> }) {
  const { lang, topic } = await params
  if (!devTopics.includes(topic as DevTopic)) return { title: 'ToolForge' }
  const copy = devCopy[lang][topic as DevTopic]
  return pageMetadata(lang, `docs/${topic}/`, `${copy.title} — ToolForge`, copy.description)
}
export default async function DevDocPage({ params }: { params: Promise<{ lang: Lang; topic: string }> }) {
  const { lang, topic: raw } = await params
  if (!devTopics.includes(raw as DevTopic)) notFound()
  const topic = raw as DevTopic
  const t = devCopy[lang][topic]
  const icons: Record<DevTopic, typeof Puzzle> = { 'plugin-development': Puzzle }
  return (
    <div className="site-wrap site-doc-layout">
      <aside className="site-doc-nav">
        <h2 className="site-sidebar-label">{siteCopy[lang].docs}</h2>
        {devTopics.map((key) => {
          const Icon = icons[key]
          return (
            <Link key={key} href={`/${lang}/docs/${key}/`} aria-current={topic === key ? 'page' : undefined}>
              <Icon aria-hidden="true" />
              {devCopy[lang][key].title}
            </Link>
          )
        })}
      </aside>
      <article className="site-doc-article site-prose">
        <nav className="site-breadcrumb" aria-label={lang === 'zh' ? '面包屑导航' : 'Breadcrumb'}>
          <Link href={`/${lang}/docs/${devTopics[0]}/`}>{siteCopy[lang].docs}</Link>
          <ChevronRight aria-hidden="true" />
          <span aria-current="page">{t.title}</span>
        </nav>
        <h1>{t.heading}</h1>
        <p className="site-intro">{t.description}</p>
        <DevMarkdown source={devArticle(topic, lang)} />
        <div className="site-doc-bottom">
          <span>ToolForge · {siteCopy[lang].docs}</span>
          <Link className="site-link" href={`/${lang}/guide/agent/`}>
            {lang === 'zh' ? '了解命令行与 AI Agent' : 'Command line and AI agents'}
            <ArrowRight aria-hidden="true" />
          </Link>
        </div>
      </article>
    </div>
  )
}
