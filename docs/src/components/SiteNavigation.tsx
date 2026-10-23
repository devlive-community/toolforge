'use client'
import Link from 'next/link'
import { usePathname } from 'next/navigation'
import { siteCopy } from '@/lib/site-copy'
import type { Lang } from '@/lib/catalog'

export function SiteNavigation({ lang, open, onNavigate }: { lang: Lang; open: boolean; onNavigate: () => void }) {
  const pathname = usePathname() ?? ''
  const t = siteCopy[lang]
  return (
    <nav id="site-navigation" className={`site-nav${open ? ' site-nav-open' : ''}`} aria-label={lang === 'zh' ? '主导航' : 'Main navigation'}>
      {[
        { href: `/${lang}/tools/`, label: t.tools },
        { href: `/${lang}/guide/start/`, prefix: `/${lang}/guide/`, label: t.guide },
        { href: `/${lang}/docs/plugin-development/`, prefix: `/${lang}/docs/`, label: t.docs },
        { href: `/${lang}/changelog/`, label: t.updates },
      ].map((item) => (
        <Link
          key={item.href}
          href={item.href}
          onClick={onNavigate}
          data-active={pathname.startsWith(item.prefix ?? item.href)}
          className="site-nav-link"
          aria-current={pathname.startsWith(item.prefix ?? item.href) ? 'page' : undefined}
        >
          {item.label}
        </Link>
      ))}
    </nav>
  )
}
