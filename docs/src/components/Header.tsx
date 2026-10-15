import Link from 'next/link'
import { Github } from 'lucide-react'
import { LangSwitch } from '@/components/LangSwitch'
import { SiteNavigation } from '@/components/SiteNavigation'
import { ThemeToggle } from '@/components/ThemeToggle'
import { REPO_URL, type Lang } from '@/lib/catalog'
import { dict } from '@/lib/dict'
import styles from './Header.module.css'

export function Header({ lang }: { lang: Lang }) {
  return (
    <header className={styles.header}>
      <div className={styles.wrap}>
        <Link href={`/${lang}/`} className={styles.brand}><img src="/icon.png" alt="" width={27} height={27} /><span>ToolForge</span></Link>
        <SiteNavigation lang={lang} />
        <div className={styles.controls}>
          <a href={REPO_URL} target="_blank" rel="noreferrer" aria-label={dict.nav.github[lang]} className={styles.github}><Github size={17} /></a>
          <LangSwitch lang={lang} />
          <ThemeToggle label={dict.theme.toggle[lang]} />
        </div>
      </div>
    </header>
  )
}
