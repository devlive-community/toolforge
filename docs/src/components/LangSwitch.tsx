'use client'

import { usePathname } from 'next/navigation'
import type { Lang } from '@/lib/catalog'

// Full navigation applies the language root and the saved theme before rendering.
export function LangSwitch({ lang }: { lang: Lang }) {
  const pathname = usePathname() ?? '/'
  const other: Lang = lang === 'zh' ? 'en' : 'zh'
  const segments = pathname.split('/')
  segments[1] = other
  const href = segments.join('/') || `/${other}/`

  return (
    <a href={href} className="site-icon-btn" aria-label={other === 'zh' ? '切换到中文' : 'Switch to English'}>
      {other === 'zh' ? '中文' : 'EN'}
    </a>
  )
}
