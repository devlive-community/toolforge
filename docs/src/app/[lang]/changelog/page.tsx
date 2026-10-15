import { release, RELEASES_URL, type Lang } from '@/lib/catalog'
import { dict } from '@/lib/dict'
import { ReleaseNotes } from '@/components/ReleaseNotes'

// 更新日志：构建时从 GitHub Releases 注入，nightly 定时重建保持新鲜
export default async function ChangelogPage({ params }: { params: Promise<{ lang: string }> }) {
  const { lang } = await params
  const langTag = lang as Lang
  const releases = release.releases ?? []

  return (
    <div className="mx-auto max-w-3xl px-4 py-12 sm:px-6">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h1 className="font-display text-3xl font-semibold tracking-tight">{dict.changelog.title[langTag]}</h1>
          <p className="mt-3 text-sm text-fg-muted">{dict.changelog.subtitle[langTag]}</p>
        </div>
        <a
          href={RELEASES_URL}
          target="_blank"
          rel="noreferrer"
          className="text-sm text-accent transition-colors hover:text-accent-strong focus-visible:outline-2 focus-visible:outline-accent"
        >
          {dict.changelog.viewAll[langTag]}
        </a>
      </div>

      {releases.length === 0 ? (
        <p className="mt-10 rounded-lg border border-dashed border-line p-10 text-center text-sm text-fg-subtle">
          {dict.changelog.empty[langTag]}
        </p>
      ) : (
        <ol className="mt-10 space-y-0 border-l border-line pl-6">
          {releases.map((item) => (
            <li key={item.tag} className="relative pb-10">
              <span className="absolute -left-[31px] top-1.5 size-2.5 rounded-full border-2 border-accent bg-bg" />
              <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
                <a
                  href={`https://github.com/devlive-community/toolforge/releases/tag/${item.tag}`}
                  target="_blank"
                  rel="noreferrer"
                  className="font-display text-lg font-semibold text-fg transition-colors hover:text-accent focus-visible:outline-2 focus-visible:outline-accent"
                >
                  {item.name}
                </a>
                <time className="text-xs text-fg-subtle tabular-nums">{item.date}</time>
              </div>
              <div className="mt-3">
                {item.body.trim() ? <ReleaseNotes body={item.body} /> : <p className="text-fg-subtle">—</p>}
              </div>
            </li>
          ))}
        </ol>
      )}
    </div>
  )
}
