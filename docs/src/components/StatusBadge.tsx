import { toolStatus, type Lang, type ToolEntry } from '@/lib/catalog'
import { dict } from '@/lib/dict'

// 状态角标：待发布（琥珀）/ 新发布（品牌绿）；已发布不显示
// 卡片与详情页共用，保证同一工具在全站状态一致
export function StatusBadge({ tool, lang, size = 'sm' }: { tool: ToolEntry; lang: Lang; size?: 'sm' | 'md' }) {
  const status = toolStatus(tool)
  if (status === 'released') return null
  const label =
    status === 'upcoming' ? dict.badge.upcoming[lang] : status === 'updated' ? dict.badge.updated[lang] : dict.badge.new[lang]
  const pad = size === 'md' ? 'px-2.5 py-1 text-[11px]' : 'px-2 py-0.5 text-[10px]'
  const tone =
    status === 'upcoming'
      ? 'border-amber/30 bg-amber/10 text-amber'
      : 'border-accent/30 bg-accent-soft text-accent-strong'
  return (
    <span
      className={`inline-flex shrink-0 items-center gap-1 whitespace-nowrap rounded-full border font-medium leading-none ${pad} ${tone}`}
    >
      {label}
    </span>
  )
}
