import { Empty } from '@toolforge/ui'
import { History as HistoryIcon } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Page } from './Page'

export function History() {
  const { t } = useTranslation()
  return (
    <Page title={t('nav.history')} description={t('history.subtitle')}>
      <Empty className="py-24" icon={<HistoryIcon />} title={t('common:comingSoon')} description={t('history.hint')} />
    </Page>
  )
}
