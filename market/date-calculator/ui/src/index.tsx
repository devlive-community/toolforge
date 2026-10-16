import { useEffect, useState } from 'react'
import { Tabs } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { CalendarDays, CalendarHeart, CalendarPlus, Hourglass, Sparkles } from 'lucide-react'
import { AddTab } from './AddTab'
import { AnniversaryTab } from './AnniversaryTab'
import { BirthTab } from './BirthTab'
import { CalendarTab } from './CalendarTab'
import { DiffTab } from './DiffTab'
import type { Today } from './types'

type Tab = 'diff' | 'add' | 'calendar' | 'birth' | 'anniversary'

function DateCalculator() {
  const { t, call } = usePlugin()
  const [tab, setTab] = useState<Tab>('diff')
  const [today, setToday] = useState<Today | null>(null)
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
        ]}
      />
      {today && (
        <div className="min-h-0 flex-1">
          <div className="h-full" hidden={tab !== 'diff'}>
            <DiffTab today={today.date} />
          </div>
          <div className="h-full" hidden={tab !== 'add'}>
            <AddTab today={today.date} />
          </div>
          <div className="h-full" hidden={tab !== 'calendar'}>
            <CalendarTab today={today} />
          </div>
          <div className="h-full" hidden={tab !== 'birth'}>
            <BirthTab today={today.date} />
          </div>
          <div className="h-full" hidden={tab !== 'anniversary'}>
            <AnniversaryTab />
          </div>
        </div>
      )}
    </div>
  )
}

export default DateCalculator
