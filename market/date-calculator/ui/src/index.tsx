import { useEffect, useState } from 'react'
import { Tabs } from '@toolforge/ui'
import { usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { CalendarCog, CalendarDays, CalendarHeart, CalendarPlus, Hourglass, Sparkles } from 'lucide-react'
import { AddTab } from './AddTab'
import { AnniversaryTab } from './AnniversaryTab'
import { BirthTab } from './BirthTab'
import { CalendarTab } from './CalendarTab'
import { DiffTab } from './DiffTab'
import { SchemesTab } from './SchemesTab'
import type { StoredScheme, Today } from './types'

type Tab = 'diff' | 'add' | 'calendar' | 'birth' | 'anniversary' | 'schemes'

function DateCalculator() {
  const { t, call } = usePlugin()
  const [tab, setTab] = useState<Tab>('diff')
  const [today, setToday] = useState<Today | null>(null)
  // 节假日方案由各页共用，在这里统一持有
  const [schemes, setSchemes] = usePluginState<StoredScheme[]>('holidaySchemes', [])
  useEffect(() => {
    call<Today>('today', {}).then(setToday, () => {})
  }, [call])

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <Tabs<Tab>
        value={tab}
        onValueChange={setTab}
        className="self-start"
        aria-label={t('name')}
        items={[
          { value: 'diff', label: t('tabs.diff'), icon: <Hourglass /> },
          { value: 'add', label: t('tabs.add'), icon: <CalendarPlus /> },
          { value: 'calendar', label: t('tabs.calendar'), icon: <CalendarDays /> },
          { value: 'birth', label: t('tabs.birth'), icon: <Sparkles /> },
          { value: 'anniversary', label: t('tabs.anniversary'), icon: <CalendarHeart /> },
          { value: 'schemes', label: t('tabs.schemes'), icon: <CalendarCog /> },
        ]}
      />
      {today && (
        <div className="min-h-0 flex-1">
          <div className="h-full" hidden={tab !== 'diff'}>
            <DiffTab today={today.date} schemes={schemes} />
          </div>
          <div className="h-full" hidden={tab !== 'add'}>
            <AddTab today={today.date} schemes={schemes} />
          </div>
          <div className="h-full" hidden={tab !== 'calendar'}>
            <CalendarTab today={today} schemes={schemes} />
          </div>
          <div className="h-full" hidden={tab !== 'birth'}>
            <BirthTab today={today.date} />
          </div>
          <div className="h-full" hidden={tab !== 'anniversary'}>
            <AnniversaryTab />
          </div>
          <div className="h-full" hidden={tab !== 'schemes'}>
            <SchemesTab schemes={schemes} onChange={setSchemes} today={today.date} />
          </div>
        </div>
      )}
    </div>
  )
}

export default DateCalculator
