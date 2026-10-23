'use client'

import { useState } from 'react'
import Link from 'next/link'
import { Menu } from 'lucide-react'
import { LangSwitch } from './LangSwitch'
import { SiteNavigation } from './SiteNavigation'
import { ThemeToggle } from './ThemeToggle'
import { DownloadDialog } from './DownloadDialog'
import { siteCopy } from '@/lib/site-copy'
import type { Lang } from '@/lib/catalog'

export function Header({ lang }: { lang: Lang }) {
  const [open, setOpen] = useState(false)
  const t = siteCopy[lang]
  return (
    <header className="site-header">
      <div className="site-wrap site-header-inner">
        <Link href={`/${lang}/`} className="site-brand" aria-label={`ToolForge ${t.home}`} onClick={() => setOpen(false)}>
          <img src="/icon.png" alt="" width={34} height={34} />
          <span>ToolForge</span>
        </Link>
        <SiteNavigation lang={lang} open={open} onNavigate={() => setOpen(false)} />
        <div className="site-header-actions">
          <LangSwitch lang={lang} />
          <ThemeToggle label={t.theme} />
          <span className="site-header-download">
            <DownloadDialog lang={lang} compact />
          </span>
          <button
            type="button"
            className="site-icon-btn site-mobile-menu"
            aria-label={t.menu}
            aria-expanded={open}
            aria-controls="site-navigation"
            onClick={() => setOpen(!open)}
          >
            <Menu aria-hidden="true" />
          </button>
        </div>
      </div>
    </header>
  )
}
