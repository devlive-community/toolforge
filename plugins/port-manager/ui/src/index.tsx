import { useState } from 'react'
import { Tabs } from '@toolforge/ui'
import { useLaunchInput, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Radar, Server } from 'lucide-react'
import { LocalPorts } from './LocalPorts'
import { RemoteCheck } from './RemoteCheck'

function PortManager() {
  const { t } = usePlugin()
  const [tab, setTab] = useState<'local' | 'remote'>('local')
  // 剪贴板中的 host:port 交给远程检查，按序号重建组件以带入初始值
  const [launched, setLaunched] = useState<{ text: string; seq: number } | null>(null)
  useLaunchInput((text) => {
    setTab('remote')
    setLaunched((prev) => ({ text, seq: (prev?.seq ?? 0) + 1 }))
  })
  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      <Tabs
        value={tab}
        onValueChange={setTab}
        className="self-start"
        items={[
          { value: 'local', label: t('tabs.local'), icon: <Server /> },
          { value: 'remote', label: t('tabs.remote'), icon: <Radar /> },
        ]}
        aria-label={t('name')}
      />
      <div className="min-h-0 flex-1">
        {tab === 'local' ? <LocalPorts /> : <RemoteCheck key={launched?.seq ?? 0} endpoint={launched?.text} />}
      </div>
    </div>
  )
}

export default PortManager
