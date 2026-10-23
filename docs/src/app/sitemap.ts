import type { MetadataRoute } from 'next'
import { catalog, LANGS, release } from '@/lib/catalog'
import { guideTopics } from '@/lib/guides'
import { RELEASE_PAGE_SIZE } from '@/components/ReleaseHistory'

export const dynamic = 'force-static'
export default function sitemap(): MetadataRoute.Sitemap {
  const paths = [
    '',
    'tools/',
    'changelog/',
    'changelog/plugins/',
    ...guideTopics.map((topic) => `guide/${topic}/`),
    ...catalog.map((tool) => `tools/${tool.slug}/`),
    ...Array.from({ length: Math.max(0, Math.ceil(release.releases.length / RELEASE_PAGE_SIZE) - 1) }, (_, i) => `changelog/page/${i + 2}/`),
  ]
  return LANGS.flatMap((lang) =>
    paths.map((path) => ({
      url: `https://toolforge.devlive.org/${lang}/${path}`,
      alternates: { languages: Object.fromEntries(LANGS.map((other) => [other, `https://toolforge.devlive.org/${other}/${path}`])) },
    })),
  )
}
