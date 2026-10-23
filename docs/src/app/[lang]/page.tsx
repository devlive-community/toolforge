import Link from 'next/link'
import { ArrowRight, ArrowUpRight, ArrowLeftRight, Code2, Globe, HardDrive, ImageIcon, Puzzle, Search, Shapes, ShieldCheck } from 'lucide-react'
import { DownloadDialog } from '@/components/DownloadDialog'
import { ToolCard } from '@/components/ToolCard'
import { getTool, type Lang } from '@/lib/catalog'
import { siteCopy } from '@/lib/site-copy'
import { pageMetadata } from '@/lib/metadata'

export async function generateMetadata({ params }: { params: Promise<{ lang: Lang }> }) {
  const { lang } = await params
  return pageMetadata(lang, '', `ToolForge — ${lang === 'zh' ? '本地优先的桌面工具箱' : 'Your local-first desktop toolkit'}`, siteCopy[lang].heroBody)
}
export default async function HomePage({ params }: { params: Promise<{ lang: Lang }> }) {
  const { lang } = await params
  const t = siteCopy[lang]
  const quick = [
    { id: 'dev', icon: Code2, zh: '开发调试', en: 'Development' },
    { id: 'image', icon: ImageIcon, zh: '图像处理', en: 'Images' },
    { id: 'convert', icon: ArrowLeftRight, zh: '文件转换', en: 'Conversions' },
    { id: 'network', icon: Globe, zh: '网络工具', en: 'Networking' },
    { id: 'other', icon: Shapes, zh: '生活与兴趣', en: 'Everyday tools' },
  ]
  const featured = ['json-formatter', 'image-compressor', 'http-client', 'ocr', 'text-diff', 'timestamp'].map(getTool).filter((item) => item !== undefined)
  return (
    <div className="site-wrap">
      <section className={`site-hero${lang === 'en' ? ' site-hero-en' : ''}`} aria-labelledby="home-title">
        <span className="site-eyebrow">
          <span className="site-dot" />
          YOUR EVERYDAY TOOLKIT
        </span>
        <h1 id="home-title">
          {t.heroFirst}
          <br />
          <span>{t.heroLast}</span>
        </h1>
        <p>{t.heroBody}</p>
        <div className="site-actions" id="download">
          <DownloadDialog lang={lang} />
          <Link className="site-secondary" href={`/${lang}/tools/`}>
            {t.tools}
            <ArrowRight aria-hidden="true" />
          </Link>
        </div>
        <div className="site-platform-note">
          macOS <span>·</span> Windows <span>·</span> Linux <span>/</span>
          {t.openLocal}
        </div>
      </section>
      <section className="site-discover" aria-label={t.search}>
        <form className="site-search" action={`/${lang}/tools/`} method="get">
          <Search aria-hidden="true" />
          <input name="q" type="search" placeholder={t.searchPlaceholder} aria-label={t.search} autoComplete="off" maxLength={512} />
          <button type="submit" className="site-key" aria-label={t.search}>
            Enter ↵
          </button>
        </form>
        <div className="site-quick">
          {quick.map(({ id, icon: Icon, ...name }) => (
            <Link key={id} href={`/${lang}/tools/?category=${id}`}>
              <Icon aria-hidden="true" />
              {name[lang]}
            </Link>
          ))}
        </div>
      </section>
      <section className="site-section">
        <div className="site-section-head">
          <div>
            <h2>{t.featured}</h2>
            <p>{t.featuredBody}</p>
          </div>
          <Link className="site-link" href={`/${lang}/tools/`}>
            {t.viewAll}
            <ArrowUpRight aria-hidden="true" />
          </Link>
        </div>
        <div className="site-grid">
          {featured.map((tool) => (
            <ToolCard key={tool.slug} tool={tool} lang={lang} />
          ))}
        </div>
      </section>
      <section className="site-section">
        <div className="site-guide-band">
          <div>
            <span className="site-eyebrow">GET STARTED</span>
            <h2>
              {t.guideHeading}
              <br />
              {t.guideHeadingEnd}
            </h2>
            <p>{t.guideBody}</p>
          </div>
          <div className="site-guide-links">
            {[
              ['start', t.guideStart],
              ['plugins', t.guidePlugins],
              ['privacy', t.guidePrivacy],
            ].map(([topic, name], index) => (
              <Link href={`/${lang}/guide/${topic}/`} key={topic}>
                <span>{String(index + 1).padStart(2, '0')}</span>
                <span>{name}</span>
                <ArrowRight aria-hidden="true" />
              </Link>
            ))}
          </div>
        </div>
        <div className="site-values">
          {[
            { icon: HardDrive, title: t.local, body: t.localBody },
            { icon: Puzzle, title: t.plugins, body: t.pluginsBody },
            { icon: ShieldCheck, title: t.permissions, body: t.permissionsBody },
          ].map(({ icon: Icon, title, body }) => (
            <div className="site-value" key={title}>
              <Icon aria-hidden="true" />
              <div>
                <h3>{title}</h3>
                <p>{body}</p>
              </div>
            </div>
          ))}
        </div>
      </section>
    </div>
  )
}
