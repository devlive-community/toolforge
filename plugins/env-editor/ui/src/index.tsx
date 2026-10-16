import { useState } from 'react'
import { Tabs } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { Columns3, FileText } from 'lucide-react'
import { Compare } from './Compare'
import { EnvEditor } from './EnvEditor'

type Tab = 'edit' | 'compare'

function EnvTool() {
  const { t } = usePlugin()
  const [tab, setTab] = useState<Tab>('edit')
  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <Tabs<Tab>
        value={tab}
        onValueChange={setTab}
        className="self-start"
        aria-label={t('name')}
        items={[
          { value: 'edit', label: t('tabs.edit'), icon: <FileText /> },
          { value: 'compare', label: t('tabs.compare'), icon: <Columns3 /> },
        ]}
      />
      <div className="min-h-0 flex-1">
        <div className="h-full" hidden={tab !== 'edit'}>
          <EnvEditor active={tab === 'edit'} />
        </div>
        <div className="h-full" hidden={tab !== 'compare'}>
          <Compare active={tab === 'compare'} />
        </div>
      </div>
    </div>
  )
}

export default EnvTool
