'use client'

import Link from 'next/link'
import { usePathname } from 'next/navigation'
import { dict } from '@/lib/dict'
import type { Lang } from '@/lib/catalog'
import styles from './Header.module.css'

export function SiteNavigation({ lang }: { lang: Lang }) {
  const pathname = usePathname() ?? ''
  return <nav className={styles.nav} aria-label={lang === 'zh' ? '主导航' : 'Main navigation'}>{[
    { href:`/${lang}/tools/`, label:dict.nav.tools[lang] },
    { href:`/${lang}/changelog/`, label:dict.nav.changelog[lang] },
  ].map((item) => <Link key={item.href} href={item.href} className={styles.navLink} data-active={pathname.startsWith(item.href)} aria-current={pathname === item.href || pathname+'/' === item.href ? 'page' : undefined}>{item.label}</Link>)}</nav>
}
