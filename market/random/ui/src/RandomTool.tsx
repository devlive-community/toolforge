import { useState } from 'react'
import { Tabs } from '@toolforge/ui'
import { usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { Dices, Hash, ListOrdered } from 'lucide-react'
import { DiceTab } from './DiceTab'
import { ListTab } from './ListTab'
import { NumbersTab } from './NumbersTab'

type Tab = 'numbers' | 'list' | 'dice'

export function RandomTool() {
  const { t } = usePlugin()
  const [tab, setTab] = useState<Tab>('numbers')
  const [seed, setSeed] = usePluginState('seed', '')

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <Tabs<Tab>
        value={tab}
        onValueChange={setTab}
        className="self-start"
        aria-label={t('name')}
        items={[
          { value: 'numbers', label: t('tabs.numbers'), icon: <Hash /> },
          { value: 'list', label: t('tabs.list'), icon: <ListOrdered /> },
          { value: 'dice', label: t('tabs.dice'), icon: <Dices /> },
        ]}
      />
      <div className="min-h-0 flex-1">
        <div className="h-full" hidden={tab !== 'numbers'}>
          <NumbersTab seed={seed} setSeed={setSeed} />
        </div>
        <div className="h-full" hidden={tab !== 'list'}>
          <ListTab seed={seed} setSeed={setSeed} />
        </div>
        <div className="h-full" hidden={tab !== 'dice'}>
          <DiceTab seed={seed} setSeed={setSeed} />
        </div>
      </div>
    </div>
  )
}
