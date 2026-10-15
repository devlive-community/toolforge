import { Download } from 'lucide-react'
import { release, RELEASES_URL, type Lang } from '@/lib/catalog'
import { dict } from '@/lib/dict'
import { getDownloadGroups } from '@/lib/downloads'

export function DownloadButtons({ lang, size = 'lg' }: { lang: Lang; size?: 'lg' | 'sm' }) {
  const t = dict.download
  const groups = getDownloadGroups()

  const cls =
    size === 'lg'
      ? 'inline-flex h-12 items-center gap-2.5 rounded-lg px-5 text-sm font-medium transition-all focus-visible:outline-2 focus-visible:outline-accent'
      : 'inline-flex h-9 items-center gap-2 rounded-md px-3.5 text-xs font-medium transition-colors focus-visible:outline-2 focus-visible:outline-accent'

  if (groups.length === 0) {
    return (
      <a
        href={RELEASES_URL+'/latest'}
        target="_blank"
        rel="noreferrer"
        className={`${cls} bg-accent text-white hover:bg-accent-strong dark:text-[#04120a]`}
      >
        <Download className={size === 'lg' ? 'size-4' : 'size-3.5'} />
        {t.other[lang]}
      </a>
    )
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap gap-x-6 gap-y-3">
        {groups.map((group) => (
          <div key={group.name} className="flex flex-col gap-1.5">
            <span className="font-display text-[10px] uppercase tracking-wider text-fg-subtle">
              {dict.download[group.name][lang]}
            </span>
            <div className="flex flex-wrap gap-2">
              {group.items.map((asset) => (
                <a
                  key={asset.url}
                  href={asset.url}
                  target="_blank"
                  rel="noreferrer"
                  className={`${cls} border border-line bg-panel text-fg hover:border-accent hover:text-accent-strong`}
                >
                  <Download className={size === 'lg' ? 'size-4' : 'size-3.5'} />
                  {t[asset.labelKey][lang]}
                </a>
              ))}
            </div>
          </div>
        ))}
      </div>
      <p className="text-xs text-fg-subtle">
        {t.version[lang]} <span className="font-display text-fg-muted">v{release.version}</span> · {t.hint[lang]}{' '}
        <a href={RELEASES_URL} target="_blank" rel="noreferrer" className="underline hover:text-fg">
          Releases
        </a>
      </p>
    </div>
  )
}
