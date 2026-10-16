import Link from 'next/link'
import { ArrowUpRight } from 'lucide-react'
import { toolDescription, toolName, toolStatus, type Lang, type ToolEntry, type ToolStatus } from '@/lib/catalog'
import styles from './ToolCard.module.css'

const labels = {
  zh: { released: '已发布', new: '新发布', updated: '已更新', upcoming: '待发布' },
  en: { released: 'Released', new: 'New', updated: 'Updated', upcoming: 'Upcoming' },
} satisfies Record<Lang, Record<ToolStatus, string>>

// Shared by the catalog and related tools on detail pages.
export function ToolCard({ tool, lang }: { tool: ToolEntry; lang: Lang }) {
  const status = toolStatus(tool)
  return (
    <Link href={`/${lang}/tools/${tool.slug}/`} className={styles.card}>
      <div className={styles.top}>
        <span className={styles.iconBox}><span className={styles.icon} aria-hidden="true" style={{ maskImage: `url(/icons/${tool.slug}.svg)`, WebkitMaskImage: `url(/icons/${tool.slug}.svg)` }} /></span>
        <span className={`${styles.badge} ${status === 'upcoming' ? styles.upcoming : status === 'released' ? styles.released : styles.fresh}`}><span aria-hidden="true" />{labels[lang][status]}</span>
      </div>
      <h3>{toolName(tool, lang)}</h3>
      <p className={styles.description}>{toolDescription(tool, lang)}</p>
      <div className={styles.bottom}><span>{tool.categoryLabel[lang]}</span><ArrowUpRight size={16} aria-hidden="true" /></div>
    </Link>
  )
}
