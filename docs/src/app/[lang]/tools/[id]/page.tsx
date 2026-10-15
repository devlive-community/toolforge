import type { Metadata } from 'next'
import Link from 'next/link'
import { notFound } from 'next/navigation'
import { ArrowLeft, Lightbulb } from 'lucide-react'
import { catalog, getTool, toolDescription, toolName, type Lang } from '@/lib/catalog'
import { dict } from '@/lib/dict'

// 65 个工具 × 双语，构建时全部静态生成
export function generateStaticParams() {
  return catalog.flatMap((tool) => [
    { lang: 'zh', id: tool.slug },
    { lang: 'en', id: tool.slug },
  ])
}

export async function generateMetadata({
  params,
}: {
  params: Promise<{ lang: string; id: string }>
}): Promise<Metadata> {
  const { lang, id } = await params
  const tool = getTool(id)
  if (!tool) return { title: 'ToolForge' }
  const name = toolName(tool, lang as Lang)
  return {
    title: `${name} · ToolForge`,
    description: toolDescription(tool, lang as Lang),
  }
}

export default async function ToolDetailPage({
  params,
}: {
  params: Promise<{ lang: string; id: string }>
}) {
  const { lang, id } = await params
  const langTag = lang as Lang
  const tool = getTool(id)
  if (!tool) notFound()
  const name = toolName(tool, langTag)
  const keywords = tool.keywords.filter((keyword) => keyword && keyword.trim() !== '')

  return (
    <article className="mx-auto max-w-3xl px-4 py-12 sm:px-6">
      <Link
        href={`/${langTag}/tools/`}
        className="inline-flex items-center gap-1.5 text-sm text-fg-muted transition-colors hover:text-fg focus-visible:outline-2 focus-visible:outline-accent"
      >
        <ArrowLeft className="size-4" />
        {dict.detail.back[langTag]}
      </Link>

      <div className="mt-6 flex items-start gap-4">
        <img
          src={`/icons/${tool.slug}.svg`}
          alt=""
          width={64}
          height={64}
          className="size-16 shrink-0 rounded-xl border border-line bg-panel p-3"
        />
        <div className="min-w-0">
          <h1 className="font-display text-2xl font-semibold tracking-tight sm:text-3xl">{name}</h1>
          <p className="mt-2 font-display text-xs uppercase tracking-wider text-fg-subtle">
            {dict.detail.category[langTag]} · {tool.categoryLabel[langTag]}
          </p>
        </div>
      </div>

      <p className="mt-6 text-[15px] leading-relaxed text-fg-muted">{toolDescription(tool, langTag)}</p>

      {keywords.length > 0 && (
        <section className="mt-8">
          <h2 className="font-display text-xs uppercase tracking-wider text-fg-subtle">
            {dict.detail.keywords[langTag]}
          </h2>
          <div className="mt-3 flex flex-wrap gap-1.5">
            {keywords.map((keyword) => (
              <span
                key={keyword}
                className="rounded-full border border-line bg-panel px-2.5 py-1 text-xs text-fg-muted"
              >
                {keyword}
              </span>
            ))}
          </div>
        </section>
      )}

      <section className="mt-10 rounded-lg border border-line bg-panel p-5">
        <h2 className="flex items-center gap-2 text-[15px] font-medium">
          <Lightbulb className="size-4 text-amber" />
          {dict.detail.howTo[langTag]}
        </h2>
        <ol className="mt-4 list-inside list-decimal space-y-2 text-[13px] leading-relaxed text-fg-muted">
          <li>{dict.detail.steps.one[langTag]}</li>
          <li>{dict.detail.steps.two[langTag]}</li>
          <li>{dict.detail.steps.three[langTag]}</li>
        </ol>
      </section>
    </article>
  )
}
