import { ReleaseHistory } from '@/components/ReleaseHistory'
import type { Lang } from '@/lib/catalog'
import { pageMetadata } from '@/lib/metadata'
import { siteCopy } from '@/lib/site-copy'
export async function generateMetadata({ params }: { params: Promise<{ lang: Lang }> }) {
  const { lang } = await params
  return pageMetadata(lang, 'changelog/', `${siteCopy[lang].updates} — ToolForge`, siteCopy[lang].releaseBody)
}
export default async function ChangelogPage({ params }: { params: Promise<{ lang: Lang }> }) {
  const { lang } = await params
  return <ReleaseHistory lang={lang} />
}
