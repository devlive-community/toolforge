import { notFound } from 'next/navigation'
import { ReleaseHistory, RELEASE_PAGE_SIZE } from '@/components/ReleaseHistory'
import { release, type Lang } from '@/lib/catalog'
import { pageMetadata } from '@/lib/metadata'
export function generateStaticParams() {
  return (['zh', 'en'] as const).flatMap((lang) =>
    Array.from({ length: Math.max(0, Math.ceil(release.releases.length / RELEASE_PAGE_SIZE) - 1) }, (_, index) => ({ lang, page: String(index + 2) })),
  )
}
export async function generateMetadata({ params }: { params: Promise<{ lang: Lang; page: string }> }) {
  const { lang, page } = await params
  return pageMetadata(
    lang,
    `changelog/page/${page}/`,
    `${lang === 'zh' ? '发布记录' : 'Release history'} · ${page} — ToolForge`,
    lang === 'zh' ? '浏览 ToolForge 历史正式发布记录。' : 'Browse official past ToolForge releases.',
  )
}
export default async function ReleasePage({ params }: { params: Promise<{ lang: Lang; page: string }> }) {
  const { lang, page: raw } = await params
  const page = Number(raw)
  if (!/^\d+$/.test(raw) || page < 2 || page > Math.ceil(release.releases.length / RELEASE_PAGE_SIZE)) notFound()
  return <ReleaseHistory lang={lang} page={page} />
}
