import { useState } from 'react'
import { Tabs } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { ArrowRightLeft, Network } from 'lucide-react'
import { RangeTool } from './RangeTool'
import { SubnetTool } from './SubnetTool'

function IpCalculator() {
  const { t } = usePlugin()
  const [tab, setTab] = useState<'subnet' | 'range'>('subnet')
  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      <Tabs
        value={tab}
        onValueChange={setTab}
        className="self-start"
        items={[
          { value: 'subnet', label: t('tabs.subnet'), icon: <Network /> },
          { value: 'range', label: t('tabs.range'), icon: <ArrowRightLeft /> },
        ]}
        aria-label={t('name')}
      />
      <div className="min-h-0 flex-1">{tab === 'subnet' ? <SubnetTool /> : <RangeTool />}</div>
    </div>
  )
}

export default IpCalculator
