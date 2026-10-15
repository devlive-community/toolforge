import Link from 'next/link'
import { Github } from 'lucide-react'
import { LangSwitch } from '@/components/LangSwitch'
import { ThemeToggle } from '@/components/ThemeToggle'
import { REPO_URL, type Lang } from '@/lib/catalog'
import { dict } from '@/lib/dict'

export function Header({ lang }: { lang: Lang }) {
  const t = dict.nav
  return (
    <header className="sticky top-0 z-40 border-b border-line bg-bg/85 backdrop-blur">
      <div className="mx-auto flex h-14 max-w-6xl items-center gap-4 px-4 sm:px-6">
        <Link href={`/${lang}/`} className="flex items-center gap-2.5 focus-visible:outline-2 focus-visible:outline-accent">
          <img src="/icon.png" alt="" width={24} height={24} className="size-6 rounded-[6px]" />
          <span className="font-display text-[15px] font-semibold tracking-tight">ToolForge</span>
        </Link>
        <nav className="ml-2 hidden items-center gap-1 text-[13px] text-fg-muted sm:flex">
          <Link className="rounded-md px-2.5 py-1.5 transition-colors hover:bg-hover hover:text-fg" href={`/${lang}/tools/`}>
            {t.tools[lang]}
          </Link>
          <Link className="rounded-md px-2.5 py-1.5 transition-colors hover:bg-hover hover:text-fg" href={`/${lang}/changelog/`}>
            {t.changelog[lang]}
          </Link>
        </nav>
        <div className="ml-auto flex items-center gap-2">
          <a
            href={REPO_URL}
            target="_blank"
            rel="noreferrer"
            aria-label={t.github[lang]}
            className="inline-flex size-9 items-center justify-center rounded-md border border-line text-fg-muted transition-colors hover:border-line-strong hover:text-fg focus-visible:outline-2 focus-visible:outline-accent"
          >
            <Github className="size-4" />
          </a>
          <LangSwitch lang={lang} />
          <ThemeToggle label={dict.theme.toggle[lang]} />
        </div>
      </div>
    </header>
  )
}
