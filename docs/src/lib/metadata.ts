import type { Metadata } from 'next'
import type { Lang } from './catalog'

export function pageMetadata(lang: Lang, path: string, title: string, description: string): Metadata {
  const route = `/${lang}/${path}`
  return {
    title,
    description,
    alternates: { canonical: route, languages: { zh: `/zh/${path}`, en: `/en/${path}` } },
    openGraph: {
      title,
      description,
      siteName: 'ToolForge',
      type: 'website',
      url: `https://toolforge.devlive.org${route}`,
      locale: lang === 'zh' ? 'zh_CN' : 'en_US',
      images: [{ url: '/og-image.png', width: 1200, height: 630, alt: 'ToolForge' }],
    },
    twitter: { card: 'summary_large_image', title, description, images: ['/og-image.png'] },
  }
}
