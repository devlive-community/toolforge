import { useState } from 'react'
import { Tabs } from '@toolforge/ui'
import { useLaunchInput, usePlugin } from '@toolforge/plugin-ui-sdk'
import { ArrowRightLeft, Network } from 'lucide-react'
import { RangeTool } from './RangeTool'
import { SubnetTool } from './SubnetTool'

function IpCalculator() {
  const { t } = usePlugin()
  const [tab, setTab] = useState<'subnet' | 'range'>('subnet')
  // 带入的地址交给子网计算，按序号重建组件以带入初始值
  const [launched, setLaunched] = useState<{ text: string; seq: number } | null>(null)
  useLaunchInput((text) => {
    setTab('subnet')
    setLaunched((prev) => ({ text, seq: (prev?.seq ?? 0) + 1 }))
  })
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
      <div className="min-h-0 flex-1">{tab === 'subnet' ? <SubnetTool key={launched?.seq ?? 0} initial={launched?.text} /> : <RangeTool />}</div>
    </div>
  )
}

export default IpCalculator
