import { ToolsBrowser } from '@/components/ToolsBrowser'
import { catalog, type Lang } from '@/lib/catalog'
import { dict } from '@/lib/dict'

export default async function ToolsPage({ params }: { params: Promise<{ lang: string }> }) {
  const { lang } = await params
  const langTag = lang as Lang
  return (
    <div className="mx-auto max-w-6xl px-4 py-12 sm:px-6">
      <h1 className="font-display text-3xl font-semibold tracking-tight">{dict.toolsPage.title[langTag]}</h1>
      <p className="mb-8 mt-3 text-sm text-fg-muted">{dict.toolsPage.subtitle[langTag]}</p>
      <ToolsBrowser tools={catalog} lang={langTag} />
    </div>
  )
}
