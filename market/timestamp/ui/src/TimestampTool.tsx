import { useEffect, useState } from 'react'
import { useLaunchInput, usePlugin } from '@toolforge/plugin-ui-sdk'
import { NowCard } from './NowCard'
import { ToDate } from './ToDate'
import { ToTimestamp } from './ToTimestamp'
import type { Now, Unit, Zones } from './types'

export function TimestampTool() {
  const { call } = usePlugin()
  const [zones, setZones] = useState<Zones>({ local: 'UTC', all: [] })
  const [value, setValue] = useState('1700000000')
  const [unit, setUnit] = useState<Unit>('auto')
  const [compare, setCompare] = useState('America/New_York')
  // 日期文本交给「日期转时间戳」，按序号重建组件以带入初始值
  const [dateInput, setDateInput] = useState<{ text: string; seq: number } | null>(null)
  useLaunchInput((text, label) => {
    if (label === 'date') {
      setDateInput((prev) => ({ text, seq: (prev?.seq ?? 0) + 1 }))
    } else {
      setValue(text)
      setUnit('auto')
    }
  })

  useEffect(() => {
    call<Zones>('timezones')
      .then((list) => {
        setZones(list)
        // 对比时区默认选一个与本机不同的常用时区
        setCompare(list.local === 'Asia/Shanghai' ? 'America/New_York' : 'Asia/Shanghai')
      })
      .catch(() => {})
  }, [call])

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <NowCard
        onUse={(seconds) => {
          setValue(String(seconds))
          setUnit('auto')
        }}
      />
      <div className="grid min-h-0 flex-1 grid-cols-2 gap-3">
        <ToDate
          value={value}
          onValueChange={setValue}
          unit={unit}
          onUnitChange={setUnit}
          compare={compare}
          onCompareChange={setCompare}
          zones={zones.all}
          onNow={() => {
            call<Now>('now')
              .then((now) => {
                setValue(String(now.seconds))
                setUnit('auto')
              })
              .catch(() => {})
          }}
        />
        <ToTimestamp key={dateInput?.seq ?? 0} initial={dateInput?.text} zones={zones.all} local={zones.local} />
      </div>
    </div>
  )
}
