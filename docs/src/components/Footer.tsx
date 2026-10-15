import Link from 'next/link'
import { REPO_URL, type Lang } from '@/lib/catalog'
import { dict } from '@/lib/dict'

export function Footer({ lang }: { lang: Lang }) {
  return (
    <footer className="border-t border-line">
      <div className="mx-auto flex max-w-6xl flex-wrap items-center gap-x-4 gap-y-2 px-4 py-8 text-xs text-fg-subtle sm:px-6">
        <span className="font-display font-medium text-fg-muted">ToolForge</span>
        <a className="transition-colors hover:text-fg" href={REPO_URL} target="_blank" rel="noreferrer">
          GitHub
        </a>
        <Link className="transition-colors hover:text-fg" href={`/${lang}/changelog/`}>
          {dict.nav.changelog[lang]}
        </Link>
        <a
          className="transition-colors hover:text-fg"
          href="https://github.com/devlive-community"
          target="_blank"
          rel="noreferrer"
        >
          devlive-community
        </a>
      </div>
    </footer>
  )
}
