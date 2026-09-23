import { Empty } from '@toolforge/ui'
import { Clock3, Star } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { pluginById, useApp } from '../stores/app'
import { Page } from './Page'
import { ToolGrid } from './ToolCard'

/** 收藏夹 / 最近使用 */
export function Collection({ kind }: { kind: 'favorites' | 'recent' }) {
  const { t } = useTranslation()
  const plugins = useApp((s) => s.plugins)
  const ids = useApp((s) => (kind === 'favorites' ? s.favorites : s.recent))
  const list = ids.map((id) => pluginById(plugins, id)).filter((p) => p !== undefined)

  return (
    <Page title={t(`nav.${kind}`)} description={t(`${kind}.subtitle`)}>
      {list.length > 0 ? (
        <ToolGrid plugins={list} />
      ) : (
        <Empty
          className="py-24"
          icon={kind === 'favorites' ? <Star /> : <Clock3 />}
          title={t(`${kind}.empty`)}
          description={t(`${kind}.emptyHint`)}
        />
      )}
    </Page>
  )
}
