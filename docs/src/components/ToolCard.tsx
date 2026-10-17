import Link from 'next/link'
import { ArrowUpRight } from 'lucide-react'
import { toolChannel, toolDescription, toolName, toolStatus, type Lang, type ToolEntry, type ToolStatus } from '@/lib/catalog'
import { InstallButton } from './InstallButton'
import styles from './ToolCard.module.css'

const labels = {
  zh: { released: '已发布', new: '新发布', updated: '已更新', upcoming: '待发布' },
  en: { released: 'Released', new: 'New', updated: 'Updated', upcoming: 'Upcoming' },
} satisfies Record<Lang, Record<ToolStatus, string>>

const channelLabels = {
  zh: { builtin: '内置', market: '在线安装' },
  en: { builtin: 'Built-in', market: 'Online' },
} satisfies Record<Lang, Record<'builtin' | 'market', string>>

// Shared by the catalog and related tools on detail pages.
// 整卡通过拉伸链接进入详情；在线插件在右下角额外提供一键安装按钮（位于链接之上）。
export function ToolCard({ tool, lang }: { tool: ToolEntry; lang: Lang }) {
  const status = toolStatus(tool)
  const channel = toolChannel(tool)
  return (
    <div className={styles.card}>
      <Link href={`/${lang}/tools/${tool.slug}/`} className={styles.cardLink} aria-label={toolName(tool, lang)} />
      <div className={styles.top}>
        <span className={styles.iconBox}><span className={styles.icon} aria-hidden="true" style={{ maskImage: `url(/icons/${tool.slug}.svg)`, WebkitMaskImage: `url(/icons/${tool.slug}.svg)` }} /></span>
        <span className={`${styles.badge} ${status === 'upcoming' ? styles.upcoming : status === 'released' ? styles.released : styles.fresh}`}><span aria-hidden="true" />{labels[lang][status]}</span>
      </div>
      <h3>{toolName(tool, lang)}</h3>
      <p className={styles.channel}><span className={channel === 'market' ? styles.marketDot : styles.builtinDot} aria-hidden="true" />{channelLabels[lang][channel]}</p>
      <p className={styles.description}>{toolDescription(tool, lang)}</p>
      <div className={styles.bottom}>
        <span>{tool.categoryLabel[lang]}</span>
        {channel === 'market' ? <InstallButton slug={tool.slug} lang={lang} variant="card" /> : <ArrowUpRight size={16} aria-hidden="true" />}
      </div>
    </div>
  )
}
