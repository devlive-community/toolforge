import Link from 'next/link'
import { ArrowUpRight } from 'lucide-react'
import { DownloadDialog } from './DownloadDialog'
import { ReleaseNotes } from './ReleaseNotes'
import { release, RELEASES_URL, type Lang } from '@/lib/catalog'
import { siteCopy } from '@/lib/site-copy'

export const RELEASE_PAGE_SIZE = 3
export function ReleaseNavigation({ lang, plugins = false }: { lang: Lang; plugins?: boolean }) {
  return (
    <nav className="site-article-tabs" aria-label={lang === 'zh' ? '发布类型' : 'Release type'}>
      <Link href={`/${lang}/changelog/`} aria-current={!plugins ? 'page' : undefined}>
        {lang === 'zh' ? '桌面应用' : 'Desktop app'}
      </Link>
      <Link href={`/${lang}/changelog/plugins/`} aria-current={plugins ? 'page' : undefined}>
        {lang === 'zh' ? '在线插件' : 'Online plugins'}
      </Link>
    </nav>
  )
}
export function ReleaseHeading({ lang }: { lang: Lang }) {
  const t = siteCopy[lang]
  return (
    <header className="site-page-heading">
      <h1>{t.releaseTitle}</h1>
      <p>{t.releaseBody}</p>
    </header>
  )
}
export function ReleaseHistory({ lang, page = 1 }: { lang: Lang; page?: number }) {
  const zh = lang === 'zh'
  const items = release.releases ?? []
  const pages = Math.max(1, Math.ceil(items.length / RELEASE_PAGE_SIZE))
  const href = (n: number) => (n === 1 ? `/${lang}/changelog/` : `/${lang}/changelog/page/${n}/`)
  return (
    <div className="site-wrap">
      <ReleaseHeading lang={lang} />
      <ReleaseNavigation lang={lang} />
      {items.length > 0 ? (
        <>
          {items.slice((page - 1) * RELEASE_PAGE_SIZE, page * RELEASE_PAGE_SIZE).map((item, i) => (
            <article className="site-release" key={item.tag} id={`release-${item.tag}`}>
              <div className="site-release-date">{item.date && <time dateTime={item.date}>{item.date}</time>}</div>
              <div>
                <h2>
                  {item.tag}
                  {page === 1 && i === 0 && !release.fallback && <span className="site-tag">{zh ? '最近发布' : 'Latest release'}</span>}
                </h2>
                {item.name !== item.tag && <p>{item.name}</p>}
                {item.summary &&
                  (['features', 'fixes', 'other'] as const).map(
                    (kind) =>
                      item.summary![kind].length > 0 && (
                        <section className="site-release-summary" key={kind}>
                          <h3>
                            {{ features: zh ? '新增功能' : 'New features', fixes: zh ? '问题修复' : 'Fixes', other: zh ? '其他改进' : 'Other changes' }[kind]}
                          </h3>
                          <ul>
                            {item.summary![kind].map((line, index) => (
                              <li key={index}>{line}</li>
                            ))}
                          </ul>
                        </section>
                      ),
                  )}
                <details className="site-release-original">
                  <summary>{zh ? '查看完整发布说明' : 'Read the complete release notes'}</summary>
                  <div>{item.body.trim() ? <ReleaseNotes body={item.body} /> : <p>{zh ? '该版本未提供发布说明。' : 'No release notes were provided.'}</p>}</div>
                </details>
                <div className="site-release-links">
                  <DownloadDialog lang={lang} compact label={zh ? '下载最新版' : 'Download latest'} />
                  <a className="site-link" href={`${RELEASES_URL}/tag/${encodeURIComponent(item.tag)}`} target="_blank" rel="noreferrer">
                    {zh ? '此版本发布页' : 'This release'}
                    <ArrowUpRight aria-hidden="true" />
                  </a>
                </div>
              </div>
            </article>
          ))}
          {pages > 1 && (
            <nav className="site-pages" aria-label={zh ? '版本列表分页' : 'Release history pages'}>
              <span>{zh ? `第 ${page} / ${pages} 页` : `Page ${page} of ${pages}`}</span>
              <div className="site-page-buttons">
                {Array.from({ length: pages }, (_, i) => i + 1).map((n) => (
                  <Link key={n} href={href(n)} aria-current={n === page ? 'page' : undefined} aria-label={zh ? `第 ${n} 页` : `Page ${n}`}>
                    {n}
                  </Link>
                ))}
              </div>
            </nav>
          )}
        </>
      ) : (
        <div className="site-empty">
          <h2>{zh ? '发布记录暂不可用' : 'Release history is unavailable'}</h2>
          <p>{zh ? '你可以前往正式发布页查看。' : 'You can still visit the official release page.'}</p>
          <a className="site-secondary" href={RELEASES_URL} target="_blank" rel="noreferrer">
            GitHub Releases
            <ArrowUpRight aria-hidden="true" />
          </a>
        </div>
      )}
    </div>
  )
}
