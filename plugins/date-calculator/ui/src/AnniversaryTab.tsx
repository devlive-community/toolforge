import { useState } from 'react'
import { Badge, Button, Empty, Input, Panel, Tooltip } from '@toolforge/ui'
import { useDebouncedCall, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { CalendarHeart, Plus, Trash2 } from 'lucide-react'
import { LunarPicker } from './LunarPicker'
import type { Anniversary, LunarDate, Upcoming } from './types'

/** 农历生日 / 纪念日倒计时（#253）：列表保存在插件状态中 */
export function AnniversaryTab() {
  const { t, errorMessage } = usePlugin()
  const [items, setItems] = usePluginState<Anniversary[]>('anniversaries', [])
  const [name, setName] = useState('')
  const [draft, setDraft] = useState<LunarDate>({ year: 2000, month: 8, day: 15, leap: false })
  const has = items.length > 0
  const key = items.map((i) => `${i.month}-${i.day}-${i.leap}`).join('|')
  const { result, error } = useDebouncedCall<Upcoming[]>(has ? 'anniversaries' : null, has ? { items: items.map(({ month, day, leap }) => ({ year: 0, month, day, leap })) } : null, [key])

  const add = () => {
    setItems([...items, { name: name.trim() || t('anniversary.unnamed'), month: draft.month, day: draft.day, leap: draft.leap }])
    setName('')
  }
  const rows = items
    .map((item, index) => ({ item, index, next: result?.[index] }))
    .sort((a, b) => (a.next?.days ?? Infinity) - (b.next?.days ?? Infinity))

  return (
    <Panel icon={<CalendarHeart />} title={t('anniversary.title')} bodyClassName="space-y-4 overflow-auto p-4">
      <div className="flex flex-wrap items-end gap-2">
        <label className="space-y-1.5">
          <span className="block text-xs font-medium text-fg-muted">{t('anniversary.name')}</span>
          <Input value={name} onChange={(event) => setName(event.target.value)} placeholder={t('anniversary.namePlaceholder')} wrapperClassName="w-56" aria-label={t('anniversary.name')} />
        </label>
        <div className="space-y-1.5">
          <span className="block text-xs font-medium text-fg-muted">{t('anniversary.date')}</span>
          <LunarPicker value={draft} onChange={setDraft} withYear={false} />
        </div>
        <Button variant="primary" onClick={add}>
          <Plus />
          {t('anniversary.add')}
        </Button>
      </div>
      {error && <p className="text-[13px] text-danger">{errorMessage(error)}</p>}
      {!has ? (
        <Empty icon={<CalendarHeart />} title={t('anniversary.empty')} description={t('anniversary.emptyHint')} />
      ) : (
        <ul className="space-y-2">
          {rows.map(({ item, index, next }) => (
            <li key={index} className="group flex items-center gap-3 rounded-card border border-border p-3">
              <div className="min-w-0 flex-1">
                <p className="text-[13px] font-medium text-fg">{item.name}</p>
                <p className="text-xs text-fg-muted">
                  {t('anniversary.every', { month: t(`lunar.months.${item.month}`), day: t(`lunar.days.${item.day}`), leap: item.leap ? t('lunar.leap') : '' })}
                  {next && ` · ${next.date} · ${next.lunar}`}
                </p>
                {next?.adjusted && <p className="text-[11px] text-warning">{t(`anniversary.adjusted.${next.adjusted}`)}</p>}
              </div>
              {next && <Badge variant={next.days === 0 ? 'success' : 'neutral'}>{next.days === 0 ? t('anniversary.today') : t('anniversary.inDays', { count: next.days })}</Badge>}
              <Tooltip content={t('anniversary.remove')}>
                <Button size="icon-sm" variant="ghost" aria-label={t('anniversary.remove')} className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100" onClick={() => setItems(items.filter((_, i) => i !== index))}>
                  <Trash2 />
                </Button>
              </Tooltip>
            </li>
          ))}
        </ul>
      )}
    </Panel>
  )
}
