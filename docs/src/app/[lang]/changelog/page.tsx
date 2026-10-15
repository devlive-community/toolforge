import type { Metadata } from 'next'
import { ArrowDownToLine, ArrowUpRight, ChevronDown, GitBranch, History } from 'lucide-react'
import Link from 'next/link'
import { ReleaseNotes } from '@/components/ReleaseNotes'
import { release, RELEASES_URL, type Lang } from '@/lib/catalog'
import styles from './changelog.module.css'

const copy = {
  zh: {
    title: '更新日志 — ToolForge',
    description: '查看 ToolForge 的正式发布记录、功能更新与修复。发布说明直接来自 GitHub Releases。',
    eyebrow: 'RELEASE NOTES',
    heading: '每一步，',
    headingAccent: '都有记录。',
    subtitle: '新工具、新体验，以及让日常使用更顺手的每一次改进。',
    github: '在 GitHub 查看全部',
    stable: '当前稳定版',
    getRelease: '获取此版本',
    versions: '版本记录',
    original: '发布说明保留原文。',
    latest: '最新发布',
    open: '展开或收起发布说明',
    releaseLink: '在 GitHub 查看此版本',
    noBody: '该版本未提供发布说明。',
    unavailable: '发布记录暂不可用',
    unavailableBody: '暂时无法加载发布记录，你可以前往 GitHub 查看完整内容。',
    unavailableAction: '前往 GitHub Releases',
    browse: '探索全部工具',
  },
  en: {
    title: 'Changelog — ToolForge',
    description: 'Explore official ToolForge releases, improvements and fixes. Release notes come directly from GitHub Releases.',
    eyebrow: 'RELEASE NOTES',
    heading: 'Every step, ',
    headingAccent: 'documented.',
    subtitle: 'New tools, thoughtful updates, and the improvements that make everyday work easier.',
    github: 'View all on GitHub',
    stable: 'Current stable release',
    getRelease: 'Get this release',
    versions: 'Release history',
    original: 'Release notes are shown in their original language.',
    latest: 'Latest release',
    open: 'Expand or collapse release notes',
    releaseLink: 'View this release on GitHub',
    noBody: 'No release notes were provided for this version.',
    unavailable: 'Release history is temporarily unavailable',
    unavailableBody: 'We could not load the release history. You can still read the full notes on GitHub.',
    unavailableAction: 'Go to GitHub Releases',
    browse: 'Explore all tools',
  },
} as const

export async function generateMetadata({ params }: { params: Promise<{ lang: string }> }): Promise<Metadata> {
  const { lang } = await params
  const text = copy[lang as Lang]
  return {
    title: text.title,
    description: text.description,
    alternates: {
      canonical: `/${lang}/changelog/`,
      languages: { zh: '/zh/changelog/', en: '/en/changelog/' },
    },
    openGraph: {
      title: text.title,
      description: text.description,
      type: 'website',
      siteName: 'ToolForge',
      url: `https://toolforge.devlive.org/${lang}/changelog/`,
      images: [{ url: '/og-image.png', width: 1200, height: 630, alt: 'ToolForge' }],
    },
    twitter: { card: 'summary_large_image', title: text.title, description: text.description, images: ['/og-image.png'] },
  }
}

// Release content is injected from GitHub at build time and rendered on the server.
export default async function ChangelogPage({ params }: { params: Promise<{ lang: string }> }) {
  const { lang: rawLang } = await params
  const lang = rawLang as Lang
  const text = copy[lang]
  const releases = release.releases ?? []
  const stableVersion = release.version.replace(/^v/, '')
  const latest = releases[0]
  const showStable = !release.fallback && stableVersion.length > 0

  return (
    <div className={styles.page}>
      <div className={styles.wrap}>
        <header className={styles.hero}>
          <div>
            <p className={styles.eyebrow}><History size={13} aria-hidden="true" />{text.eyebrow}</p>
            <h1>{text.heading}<span>{text.headingAccent}</span></h1>
            <p className={styles.subtitle}>{text.subtitle}</p>
          </div>
          <a className={styles.githubLink} href={RELEASES_URL} target="_blank" rel="noreferrer">{text.github}<ArrowUpRight size={15} aria-hidden="true" /></a>
        </header>

        {releases.length === 0 ? (
          <section className={styles.empty} aria-labelledby="release-unavailable">
            <span className={styles.emptyIcon}><History size={27} aria-hidden="true" /></span>
            <h2 id="release-unavailable">{text.unavailable}</h2>
            <p>{text.unavailableBody}</p>
            <a className={styles.emptyAction} href={RELEASES_URL} target="_blank" rel="noreferrer">{text.unavailableAction}<ArrowUpRight size={15} aria-hidden="true" /></a>
          </section>
        ) : (
          <div className={styles.content}>
            <aside className={styles.sidebar}>
              <div className={styles.sidebarInner}>
                {showStable && (
                  <section className={styles.stable} aria-labelledby="stable-heading">
                    <p id="stable-heading" className={styles.stableLabel}><span />{text.stable}</p>
                    <p className={styles.stableVersion}>v{stableVersion}</p>
                    {latest?.date && <time className={styles.stableDate} dateTime={latest.date}>{latest.date}</time>}
                    <a href={release.url || RELEASES_URL} target="_blank" rel="noreferrer">{text.getRelease}<ArrowDownToLine size={13} aria-hidden="true" /></a>
                  </section>
                )}
                <nav className={styles.versionNav} aria-label={text.versions}>
                  <h2>{text.versions}</h2>
                  <ol>
                    {releases.map((item, index) => (
                      <li key={item.tag}>
                        <a href={`#${encodeURIComponent(`release-${item.tag}`)}`}>
                          <span className={styles.versionTag}><span className={index === 0 ? styles.latestDot : styles.versionDot} />{item.tag}</span>
                          {item.date && <time dateTime={item.date}>{item.date}</time>}
                        </a>
                      </li>
                    ))}
                  </ol>
                </nav>
                <p className={styles.originalNote}>{text.original}</p>
                <Link className={styles.toolsLink} href={`/${lang}/tools/`}>{text.browse}<ArrowUpRight size={13} aria-hidden="true" /></Link>
              </div>
            </aside>

            <ol className={styles.timeline} aria-label={text.versions}>
              {releases.map((item, index) => (
                <li key={item.tag} className={styles.timelineItem}>
                  <span className={index === 0 ? styles.timelineDotLatest : styles.timelineDot} aria-hidden="true" />
                  <details id={`release-${item.tag}`} className={styles.release} open={index < 3}>
                    <summary className={styles.summary} aria-label={`${item.tag} — ${text.open}`}>
                      <span className={styles.summaryContent}>
                        <span className={styles.releaseMeta}>
                          {item.date && <time dateTime={item.date}>{item.date}</time>}
                          {index === 0 && !release.fallback && <span className={styles.latestBadge}>{text.latest}</span>}
                        </span>
                        <span className={styles.releaseHeading}>{item.tag}</span>
                        {item.name && item.name !== item.tag && <span className={styles.releaseName}>{item.name}</span>}
                      </span>
                      <span className={styles.expandIcon}><ChevronDown size={19} aria-hidden="true" /></span>
                    </summary>
                    <div className={styles.releaseBody}>
                      <div className={styles.notes}>
                        {item.body.trim() ? <ReleaseNotes body={item.body} /> : <p className={styles.noBody}>{text.noBody}</p>}
                      </div>
                      <a className={styles.releaseLink} href={`${RELEASES_URL}/tag/${encodeURIComponent(item.tag)}`} target="_blank" rel="noreferrer"><GitBranch size={13} aria-hidden="true" />{text.releaseLink}<ArrowUpRight size={13} aria-hidden="true" /></a>
                    </div>
                  </details>
                </li>
              ))}
            </ol>
          </div>
        )}
      </div>
    </div>
  )
}
