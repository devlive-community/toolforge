import Link from 'next/link'
import { ArrowUpRight } from 'lucide-react'
import { REPO_URL, RELEASES_URL, type Lang } from '@/lib/catalog'
import { siteCopy } from '@/lib/site-copy'

export function Footer({ lang }: { lang: Lang }) {
  const t = siteCopy[lang]
  return (
    <footer className="site-footer site-wrap">
      <div>
        <strong>ToolForge</strong>
        <p>{t.tagline}</p>
      </div>
      <div className="site-footer-links">
        <Link href={`/${lang}/guide/start/`}>{t.guide}</Link>
        <Link href={`/${lang}/guide/privacy/`}>{t.privacy}</Link>
        <a href={RELEASES_URL} target="_blank" rel="noreferrer">
          Releases
        </a>
        <a href={REPO_URL} target="_blank" rel="noreferrer">
          GitHub <ArrowUpRight aria-hidden="true" />
        </a>
      </div>
    </footer>
  )
}
