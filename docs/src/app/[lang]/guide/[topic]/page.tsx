import Link from 'next/link'
import { notFound } from 'next/navigation'
import { ArrowRight, Bot, ChevronRight, CircleHelp, ListChecks, Puzzle, Rocket, ShieldCheck } from 'lucide-react'
import { GuideContent } from '@/components/GuideContent'
import { guideCopy, guideTopics, type GuideTopic } from '@/lib/guides'
import type { Lang } from '@/lib/catalog'
import { pageMetadata } from '@/lib/metadata'
import { siteCopy } from '@/lib/site-copy'

export function generateStaticParams() {
  return (['zh', 'en'] as const).flatMap((lang) => guideTopics.map((topic) => ({ lang, topic })))
}
export async function generateMetadata({ params }: { params: Promise<{ lang: Lang; topic: string }> }) {
  const { lang, topic } = await params
  if (!guideTopics.includes(topic as GuideTopic)) return { title: 'ToolForge' }
  const copy = guideCopy[lang][topic as GuideTopic]
  return pageMetadata(lang, `guide/${topic}/`, `${copy.title} — ToolForge`, copy.description)
}
export default async function GuidePage({ params }: { params: Promise<{ lang: Lang; topic: string }> }) {
  const { lang, topic: raw } = await params
  if (!guideTopics.includes(raw as GuideTopic)) notFound()
  const topic = raw as GuideTopic
  const t = guideCopy[lang][topic]
  const icons = { start: Rocket, plugins: Puzzle, tasks: ListChecks, privacy: ShieldCheck, faq: CircleHelp, agent: Bot }
  return (
    <div className="site-wrap site-doc-layout">
      <aside className="site-doc-nav">
        <h2 className="site-sidebar-label">{lang === 'zh' ? '使用指南' : 'Guides'}</h2>
        {guideTopics.map((key) => {
          const Icon = icons[key]
          return (
            <Link key={key} href={`/${lang}/guide/${key}/`} aria-current={topic === key ? 'page' : undefined}>
              <Icon aria-hidden="true" />
              {guideCopy[lang][key].title}
            </Link>
          )
        })}
      </aside>
      <article className="site-doc-article site-prose">
        <nav className="site-breadcrumb" aria-label={lang === 'zh' ? '面包屑导航' : 'Breadcrumb'}>
          <Link href={`/${lang}/guide/start/`}>{siteCopy[lang].guide}</Link>
          <ChevronRight aria-hidden="true" />
          <span aria-current="page">{t.title}</span>
        </nav>
        <h1>{t.heading}</h1>
        <p className="site-intro">{t.description}</p>
        <GuideContent topic={topic} lang={lang} />
        <div className="site-doc-bottom">
          <span>ToolForge · {siteCopy[lang].guide}</span>
          <Link className="site-link" href={`/${lang}/${topic === 'agent' ? 'tools/' : 'guide/agent/'}`}>
            {topic === 'agent' ? siteCopy[lang].tools : guideCopy[lang].agent.title}
            <ArrowRight aria-hidden="true" />
          </Link>
        </div>
      </article>
    </div>
  )
}
