import Link from 'next/link'
import { ArrowRight, Puzzle, ShieldCheck, Sparkles } from 'lucide-react'
import { DownloadButtons } from '@/components/DownloadButtons'
import { ToolCard } from '@/components/ToolCard'
import { catalog, type Lang } from '@/lib/catalog'
import { dict } from '@/lib/dict'

// 首页工具精选的数量
const FEATURED = 8

export default async function HomePage({ params }: { params: Promise<{ lang: string }> }) {
  const { lang: rawLang } = await params
  const lang = rawLang as Lang
  const hero = dict.hero
  const categories = new Set(catalog.map((tool) => tool.category))
  const featured = catalog.slice(0, FEATURED)

  return (
    <div>
      {/* Hero：点阵网格背景 + 品牌绿标题 + 平台下载 */}
      <section className="blueprint border-b border-line">
        <div className="mx-auto max-w-6xl px-4 py-20 sm:px-6 sm:py-28">
          <p className="mb-5 inline-flex items-center gap-2 rounded-full border border-line bg-panel px-3.5 py-1.5 text-xs text-fg-muted">
            <span className="size-1.5 rounded-full bg-amber" />
            {hero.badge[lang]}
          </p>
          <h1 className="max-w-2xl font-display text-4xl font-semibold leading-tight tracking-tight sm:text-5xl">
            {hero.title[lang]}
            <span className="text-accent">{hero.titleAccent[lang]}</span>
          </h1>
          <p className="mt-5 max-w-2xl text-[15px] leading-relaxed text-fg-muted">{hero.subtitle[lang]}</p>
          <div className="mt-8">
            <DownloadButtons lang={lang} />
          </div>
          <div className="mt-8 flex flex-wrap gap-4 text-sm">
            <Link
              href={`/${lang}/tools/`}
              className="inline-flex items-center gap-1.5 font-medium text-accent transition-colors hover:text-accent-strong focus-visible:outline-2 focus-visible:outline-accent"
            >
              {hero.browse[lang]}
              <ArrowRight className="size-4" />
            </Link>
            <a
              href="https://github.com/devlive-community/toolforge"
              target="_blank"
              rel="noreferrer"
              className="inline-flex items-center gap-1.5 text-fg-muted transition-colors hover:text-fg focus-visible:outline-2 focus-visible:outline-accent"
            >
              {hero.viewSource[lang]}
            </a>
          </div>
        </div>
      </section>

      {/* 统计条 */}
      <section className="border-b border-line bg-bg-subtle">
        <div className="mx-auto grid max-w-6xl grid-cols-3 divide-x divide-line px-4 sm:px-6">
          <Stat value={String(catalog.length)} label={dict.stats.tools[lang]} />
          <Stat value={String(categories.size)} label={dict.stats.categories[lang]} />
          <Stat value="0" label={dict.stats.uploads[lang]} />
        </div>
      </section>

      {/* 特性 */}
      <section className="mx-auto max-w-6xl px-4 py-16 sm:px-6">
        <h2 className="font-display text-2xl font-semibold tracking-tight">{dict.features.title[lang]}</h2>
        <div className="mt-8 grid gap-4 sm:grid-cols-3">
          <Feature
            icon={<ShieldCheck className="size-5" />}
            title={dict.features.privacyTitle[lang]}
            body={dict.features.privacyBody[lang]}
          />
          <Feature
            icon={<Puzzle className="size-5" />}
            title={dict.features.pluginTitle[lang]}
            body={dict.features.pluginBody[lang]}
          />
          <Feature
            icon={<Sparkles className="size-5" />}
            title={dict.features.openTitle[lang]}
            body={dict.features.openBody[lang]}
          />
        </div>
      </section>

      {/* 精选工具 */}
      <section className="border-t border-line bg-bg-subtle">
        <div className="mx-auto max-w-6xl px-4 py-16 sm:px-6">
          <div className="flex items-end justify-between gap-4">
            <h2 className="font-display text-2xl font-semibold tracking-tight">{dict.nav.tools[lang]}</h2>
            <Link
              href={`/${lang}/tools/`}
              className="inline-flex items-center gap-1.5 text-sm text-accent transition-colors hover:text-accent-strong focus-visible:outline-2 focus-visible:outline-accent"
            >
              {dict.hero.browse[lang]}
              <ArrowRight className="size-4" />
            </Link>
          </div>
          <div className="mt-8 grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
            {featured.map((tool) => (
              <ToolCard key={tool.slug} tool={tool} lang={lang} />
            ))}
          </div>
        </div>
      </section>
    </div>
  )
}

function Stat({ value, label }: { value: string; label: string }) {
  return (
    <div className="flex flex-col items-center gap-1 py-8">
      <span className="font-display text-3xl font-semibold text-accent tabular-nums">{value}</span>
      <span className="text-xs text-fg-muted">{label}</span>
    </div>
  )
}

function Feature({ icon, title, body }: { icon: React.ReactNode; title: string; body: string }) {
  return (
    <div className="rounded-lg border border-line bg-panel p-5 transition-colors hover:border-line-strong">
      <div className="flex size-10 items-center justify-center rounded-lg bg-accent-soft text-accent-strong">
        {icon}
      </div>
      <h3 className="mt-4 text-[15px] font-medium">{title}</h3>
      <p className="mt-2 text-[13px] leading-relaxed text-fg-muted">{body}</p>
    </div>
  )
}
