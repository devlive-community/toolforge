import { Empty } from '@toolforge/ui'
import { Puzzle } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { pluginById, useApp } from '../stores/app'
import { useHiddenTools } from '../stores/prefs'
import { Page, Section } from './Page'
import { ToolGrid } from './ToolCard'

export function Home() {
  const { t } = useTranslation()
  const all = useApp((s) => s.plugins)
  const hidden = useHiddenTools()
  const plugins = all.filter((p) => !hidden.includes(p.id))
  const recent = useApp((s) => s.recent)
  const recentPlugins = recent.map((id) => pluginById(plugins, id)).filter((p) => p !== undefined).slice(0, 4)

  return (
    <Page title={t('home.title')} description={t('home.subtitle')}>
      {recentPlugins.length > 0 && (
        <Section title={t('nav.recent')}>
          <ToolGrid plugins={recentPlugins} />
        </Section>
      )}
      <Section title={t('home.allTools', { count: plugins.length })}>
        {plugins.length > 0 ? (
          <ToolGrid plugins={plugins} />
        ) : (
          <Empty icon={<Puzzle />} title={t('home.noPlugins')} description={t('home.noPluginsHint')} />
        )}
      </Section>
    </Page>
  )
}
