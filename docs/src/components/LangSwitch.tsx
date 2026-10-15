'use client'

import Link from 'next/link'
import { usePathname } from 'next/navigation'
import type { Lang } from '@/lib/catalog'

// 同路径在 zh / en 之间切换
export function LangSwitch({ lang }: { lang: Lang }) {
  const pathname = usePathname() ?? '/'
  const other: Lang = lang === 'zh' ? 'en' : 'zh'
  const segments = pathname.split('/')
  segments[1] = other
  const href = segments.join('/') || `/${other}/`

  return (
    <Link
      href={href}
      className="inline-flex h-9 items-center rounded-md border border-line px-3 font-display text-xs uppercase tracking-wider text-fg-muted transition-colors hover:border-line-strong hover:text-fg focus-visible:outline-2 focus-visible:outline-accent"
    >
      {other === 'zh' ? '中文' : 'EN'}
    </Link>
  )
}
