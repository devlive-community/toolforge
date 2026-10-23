import { toolChannel, toolStatus, type Lang, type ToolEntry } from '@/lib/catalog'

export function StatusBadge({ tool, lang }: { tool: ToolEntry; lang: Lang; size?: 'sm' | 'md' }) {
  if (toolChannel(tool) === 'market' && tool.releaseVerified === false)
    return (
      <span className="site-card-status" data-pending="true">
        {lang === 'zh' ? '发布信息待确认' : 'Release unverified'}
      </span>
    )
  const status = toolStatus(tool)
  if (status === 'released') return null
  const label =
    status === 'upcoming'
      ? tool.updateOnly
        ? lang === 'zh'
          ? '有待发布更新'
          : 'Update pending'
        : lang === 'zh'
          ? '待发布'
          : 'Upcoming'
      : status === 'updated'
        ? lang === 'zh'
          ? '已更新'
          : 'Updated'
        : lang === 'zh'
          ? '新发布'
          : 'New'
  return (
    <span className="site-card-status" data-pending={status === 'upcoming'}>
      {label}
    </span>
  )
}
