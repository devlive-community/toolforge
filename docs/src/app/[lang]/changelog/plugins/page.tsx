import { ReleaseHeading, ReleaseNavigation } from '@/components/ReleaseHistory'
import { PluginVersions } from '@/components/PluginVersions'
import { catalog, type Lang } from '@/lib/catalog'
import { pageMetadata } from '@/lib/metadata'
export async function generateMetadata({ params }: { params: Promise<{ lang: Lang }> }) {
  const { lang } = await params
  return pageMetadata(
    lang,
    'changelog/plugins/',
    `${lang === 'zh' ? '已发布插件版本' : 'Published plugin versions'} — ToolForge`,
    lang === 'zh' ? '查看 ToolForge 在线插件当前已发布的版本与使用文档。' : 'Browse the current published versions and guides for ToolForge online plugins.',
  )
}
export default async function PluginReleases({ params }: { params: Promise<{ lang: Lang }> }) {
  const { lang } = await params
  return (
    <div className="site-wrap">
      <ReleaseHeading lang={lang} />
      <ReleaseNavigation lang={lang} plugins />
      <PluginVersions tools={catalog} lang={lang} />
    </div>
  )
}
