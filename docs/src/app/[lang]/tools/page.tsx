import type { Metadata } from 'next'
import { ToolsBrowser } from '@/components/ToolsBrowser'
import { catalog, type Lang } from '@/lib/catalog'
import styles from './tools.module.css'

const copy = {
  zh: {
    title: '找到手边需要的工具。',
    subtitle: '从开发调试到日常效率，把繁琐的事情，交给顺手的工具。',
    tools: '个工具',
    categories: '个分类',
    note: '持续生长的本地工具箱',
  },
  en: {
    title: 'Find the right tool for the task.',
    subtitle: 'From development to everyday tasks, make the busywork a little easier.',
    tools: 'tools',
    categories: 'categories',
    note: 'A growing toolbox that stays local',
  },
} satisfies Record<Lang, Record<string, string>>

export async function generateMetadata({ params }: { params: Promise<{ lang: string }> }): Promise<Metadata> {
  const { lang } = await params
  const zh = lang !== 'en'
  const langTag = zh ? 'zh' : 'en'
  const title = zh ? '全部工具 — ToolForge' : 'All tools — ToolForge'
  const description = zh
    ? '按分类、关键词和发布状态浏览 ToolForge 工具目录，了解开发、图像、文本、网络等本地工具。'
    : 'Explore the ToolForge catalog by category, keyword and release status. Discover local tools for development, images, text, networking and more.'

  return {
    title,
    description,
    alternates: {
      canonical: `/${langTag}/tools/`,
      languages: { zh: '/zh/tools/', en: '/en/tools/' },
    },
    openGraph: {
      title,
      description,
      type: 'website',
      siteName: 'ToolForge',
      url: `https://toolforge.devlive.org/${langTag}/tools/`,
      images: [{ url: '/og-image.png', width: 1200, height: 630, alt: 'ToolForge' }],
    },
    twitter: { card: 'summary_large_image', title, description, images: ['/og-image.png'] },
  }
}

export default async function ToolsPage({ params }: { params: Promise<{ lang: string }> }) {
  const { lang } = await params
  const langTag = lang as Lang
  const t = copy[langTag]
  const categoryCount = new Set(catalog.map((tool) => tool.category)).size

  return (
    <div className={styles.page}>
      <div className={styles.wrap}>
        <header className={styles.hero}>
          <div>
            <p className={styles.eyebrow}>THE TOOLBOX</p>
            <h1>{t.title}</h1>
            <p className={styles.subtitle}>{t.subtitle}</p>
          </div>
          <div className={styles.catalogMeta}>
            <div className={styles.numbers}>
              <p><strong>{catalog.length}</strong><span>{t.tools}</span></p>
              <span className={styles.divider} aria-hidden="true" />
              <p><strong>{categoryCount}</strong><span>{t.categories}</span></p>
            </div>
            <p className={styles.note}><span aria-hidden="true" />{t.note}</p>
          </div>
        </header>
        <ToolsBrowser tools={catalog} lang={langTag} />
      </div>
    </div>
  )
}
